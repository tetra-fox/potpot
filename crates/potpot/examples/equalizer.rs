//! An equalizer on a lit screen, opened by `just equalizer`.
//!
//! The screen shows the curve the three bands make together, over a spectrum that a thread
//! standing in for a plugin's audio thread writes through a [`Feed`]. Each band is a dot: a drag
//! moves its frequency and its gain, the wheel over it turns its Q, and a double click puts it
//! back. While a dot is held the screen writes out where its band stands.
//!
//! The dots' parameters lie along the same [`Axis`] each as the curve is drawn along, so a dot
//! is at its band's frequency and gain in the picture. A bell alone has its dot on the curve.
//! A shelf's dot is at the gain the curve levels out at, and the other bands move the curve
//! under any dot.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;
use std::time::Duration;

use potpot::glam::{Mat4, Vec2, Vec3};
use potpot::parts::{self, Screen};
use potpot::{Light, Look, Material, Mesh, Object, Place, Renderer, Scene};
use potpot_window::controls::axis::Axis;
use potpot_window::controls::flat::{Color, Flat};
use potpot_window::controls::interaction::{Control, Dial, Readout, Target};
use potpot_window::controls::response::{self, Band, Kind};
use potpot_window::{Feed, Gpu, Opening, Param, Settings, Skin, View, wgpu};

const SIZE: [f32; 2] = [720.0, 424.0];
/// The middle of the screen.
const SCREEN: Vec2 = Vec2::new(360.0, 218.0);
/// The width and length of the screen's face.
const FACE: Vec2 = Vec2::new(640.0, 300.0);
/// The distance from the edge of the face in to the picture.
const INSET: f32 = 14.0;
/// The color of the face.
const DARK: Vec3 = Vec3::new(0.012, 0.016, 0.018);

/// The shape of each band, from the lowest.
const KINDS: [Kind; 3] = [Kind::LowShelf, Kind::Bell, Kind::HighShelf];
/// The frequency in hertz each band starts at.
const STARTS: [f32; 3] = [120.0, 1000.0, 6000.0];
// a band's three parameters, counted from its first. the panel's parameters are each band's
// three in turn, and the control at a band's place is its dot
const FREQUENCY: usize = 0;
const GAIN: usize = 1;
const Q: usize = 2;
const EACH: usize = 3;
/// The count of parameters.
const PARAMETERS: usize = KINDS.len() * EACH;

// what the picture is drawn along and the parameters lie along: hertz across and decibels up,
// and the run of a band's q
const HERTZ: Axis = Axis::log([20.0, 20000.0]);
const DECIBELS: Axis = Axis::linear([-18.0, 18.0]);
const QS: Axis = Axis::log([0.3, 8.0]);
/// The decibels the spectrum shows, from its foot to its top.
const LOUDNESS: Axis = Axis::linear([-60.0, 0.0]);
/// The sample rate the curve is drawn for, in hertz.
const RATE: f32 = 48000.0;
/// The count of slots in the feed.
///
/// Each is how loud the sound is at one frequency, in decibels.
const BINS: usize = 64;
/// The time between two blocks of the made-up sound.
const BLOCK: Duration = Duration::from_millis(30);

/// How far from its middle a band's dot reaches.
const DOT: f32 = 6.0;
const GRID: Color = [0.09, 0.13, 0.15, 1.0];
const INK: Color = [0.3, 0.42, 0.46, 1.0];
const SPECTRUM: Color = [0.04, 0.2, 0.24, 1.0];
/// The count of objects that show the spectrum.
///
/// The spectrum is in one color.
const SPECTRUM_OBJECTS: usize = 1;
const CURVE: Color = [0.45, 0.95, 1.0, 1.0];
const AMBER: Color = [1.0, 0.72, 0.2, 1.0];

/// Returns the bands as the parameters have them.
///
/// `values` is where each parameter stands, as a part of its whole turn.
fn bands(values: &[f32]) -> [Band; KINDS.len()] {
  std::array::from_fn(|band| {
    let value = |parameter: usize| values[band * EACH + parameter];
    Band {
      kind: KINDS[band],
      frequency: HERTZ.plain(value(FREQUENCY)),
      gain: DECIBELS.plain(value(GAIN)),
      q: QS.plain(value(Q)),
    }
  })
}

/// Returns every parameter, and a dot for each band.
fn wiring() -> (Vec<Dial>, Vec<Control>) {
  let dials = (STARTS.iter())
    .flat_map(|&start| {
      [
        Dial::along(HERTZ, start, None, 0, " Hz"),
        Dial::along(DECIBELS, 0.0, None, 1, " dB"),
        Dial::along(QS, 0.8, None, 2, ""),
      ]
    })
    .collect();
  // a dot is dragged over the whole picture, so it keeps to the pointer, and the wheel is on
  // its band's q
  let room = (FACE - Vec2::splat(INSET * 2.0)).into();
  let controls = (0..KINDS.len())
    .map(|band| {
      let first = band * EACH;
      let mut dot = Control::pad(first + FREQUENCY, first + GAIN, room, Readout::new(5));
      dot.wheel = Some(first + Q);
      dot
    })
    .collect();
  (dials, controls)
}

/// Returns the point of the panel at a place in the picture.
///
/// The place is a part of the way across the picture and a part of the way up it.
fn place([across, up]: [f32; 2]) -> [f32; 2] {
  let half = FACE / 2.0 - Vec2::splat(INSET);
  let (min, size) = (SCREEN - half, half * 2.0);
  [min.x + across * size.x, min.y + (1.0 - up) * size.y]
}

/// Returns how far across the picture a slot of the feed is.
fn across(bin: usize) -> f32 {
  bin as f32 / (BINS - 1) as f32
}

/// Returns the spectrum as a filled region.
///
/// The region lies under how loud each frequency is. `feed` is how loud the sound is at each
/// of its frequencies, in decibels.
fn spectrum(feed: &[f32]) -> Flat {
  let mut flat = Flat::default();
  let edge: Vec<[f32; 2]> = (feed.iter().enumerate())
    .map(|(bin, loud)| place([across(bin), LOUDNESS.turn(*loud)]))
    .collect();
  flat.area(&edge, place([0.0, 0.0])[1], SPECTRUM);
  flat
}

/// Returns the lines and lettering that divide the picture.
///
/// There is a line for each tenfold step of frequency and for each six decibels.
fn grid() -> Flat {
  let mut flat = Flat::default();
  let named = [(100.0, "100"), (1000.0, "1K"), (10000.0, "10K")];
  for (frequency, _) in named {
    let across = HERTZ.turn(frequency);
    flat.line(place([across, 0.0]), place([across, 1.0]), 1.0, GRID);
  }
  for gain in [-12.0, -6.0, 0.0, 6.0, 12.0] {
    let up = DECIBELS.turn(gain);
    flat.line(place([0.0, up]), place([1.0, up]), 1.0, GRID);
  }
  // after every line, so that the two colors are two objects on the screen
  for (frequency, name) in named {
    let [x, y] = place([HERTZ.turn(frequency), 0.0]);
    flat.legend(name, [x + 4.0, y - 10.0], 6.0, 1.0, INK);
  }
  flat
}

/// Returns the bands' curve with a dot for each band.
///
/// `values` is where each parameter stands and `held` is the dot that is held down, whose
/// band is written out.
fn curve(values: &[f32], held: Option<usize>) -> Flat {
  let mut flat = Flat::default();
  let bands = bands(values);
  let points: Vec<[f32; 2]> = response::trace(&bands, RATE, HERTZ, DECIBELS, 240)
    .into_iter()
    .map(place)
    .collect();
  flat.curve(&points, 2.2, CURVE);
  let dots = dots(values);
  for dot in dots {
    flat.line(dot, dot, DOT * 2.0, AMBER);
  }
  if let Some(band) = held.map(|band| bands[band]) {
    let stands = format!(
      "{:.0} HZ  {:+.1} DB  Q {:.2}",
      band.frequency, band.gain, band.q
    );
    flat.legend(&stands, place([0.015, 0.95]), 7.0, 1.2, AMBER);
  }
  // a dot is a ring until it is held
  for (band, dot) in dots.into_iter().enumerate() {
    if held != Some(band) {
      flat.line(dot, dot, DOT, [0.1, 0.06, 0.01, 1.0]);
    }
  }
  flat
}

/// Returns where each band's dot is.
///
/// A dot's two parameters lie along the picture's own axes, so they are its place in the
/// picture: across at its band's frequency and up at its band's gain.
fn dots(values: &[f32]) -> [[f32; 2]; KINDS.len()] {
  std::array::from_fn(|band| {
    let first = band * EACH;
    place([values[first + FREQUENCY], values[first + GAIN]])
  })
}

/// A picture on the screen, with the objects that show it.
///
/// It keeps what it was drawn from. The picture is drawn and its objects are made again only
/// from something else. A frame with the same pictures costs the renderer no drawing, and one
/// with a new spectrum draws the spectrum alone and makes one object.
struct Shown<From> {
  from: Option<From>,
  objects: Vec<Object>,
}

impl<From: PartialEq> Shown<From> {
  fn new() -> Self {
    Self {
      from: None,
      objects: Vec::new(),
    }
  }

  /// Shows the picture that `flat` draws from `from`.
  ///
  /// It goes over `under` objects already on the screen.
  fn show(&mut self, from: From, flat: impl FnOnce(&From) -> Flat, glass: Material, under: usize) {
    if self.from.as_ref() != Some(&from) {
      self.objects = parts::picture(&flat(&from), SCREEN.x, SCREEN.y, glass, under);
      self.from = Some(from);
    }
  }
}

/// The panel as potpot draws it.
struct Equalizer {
  renderer: Renderer,
  screen: Screen,
  glass: Material,
  /// Everything that never changes: the plate and the screen.
  fixed: Vec<Object>,
  /// The lines and lettering on the screen, over the spectrum.
  grid: Vec<Object>,
  /// The spectrum, drawn from how loud each frequency is.
  spectrum: Shown<Vec<f32>>,
  /// The curve, drawn from the parameters and the dot held down.
  curve: Shown<(Vec<f32>, Option<usize>)>,
  /// Where each band's dot was last drawn.
  dots: [[f32; 2]; KINDS.len()],
}

impl Equalizer {
  fn new(gpu: &Gpu) -> Self {
    let put = |mesh: Mesh, at: Vec2, material: Material| Object {
      mesh: Arc::new(mesh),
      transform: Mat4::from_translation(at.extend(0.0)),
      material,
    };
    let size = Vec2::from(SIZE);
    let screen = Screen::new(FACE.x, FACE.y);
    // large for the face, so that the sheen of its glass lies over one corner alone
    let glass = Material::screen(DARK, (FACE.x + FACE.y) * 3.0);
    let chrome = Material::chrome(Vec3::splat(0.8));
    let mut fixed = vec![
      put(
        parts::plate(size.x, size.y, 6.0),
        Vec2::ZERO,
        Material::textured(Vec3::new(0.075, 0.08, 0.09)),
      ),
      put(
        parts::legend("EQUALIZER", 9.0, 1.6),
        Vec2::new(size.x / 2.0, 24.0),
        Material::matte(Vec3::splat(0.8)),
      ),
      put(screen.face(), SCREEN, glass),
      put(screen.bezel(), SCREEN, Material::glossy(Vec3::splat(0.035))),
    ];
    let inset = 16.0;
    let corners = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)];
    for corner in corners.map(Vec2::from) {
      let at = Vec2::splat(inset) + corner * (size - Vec2::splat(inset * 2.0));
      fixed.push(put(
        parts::screw_seat(6.0, 1.5),
        at,
        Material::matte(Vec3::splat(0.012)),
      ));
      let turned = Mat4::from_rotation_z(at.x * 0.3 + at.y);
      fixed.push(put(parts::screw(6.0).transformed(turned), at, chrome));
    }
    Self {
      renderer: Renderer::new(gpu.device, gpu.format),
      screen,
      glass,
      fixed,
      grid: parts::picture(&grid(), SCREEN.x, SCREEN.y, glass, SPECTRUM_OBJECTS),
      spectrum: Shown::new(),
      curve: Shown::new(),
      dots: [[0.0; 2]; KINDS.len()],
    }
  }

  /// Returns the scene with the sound and bands on its screen.
  ///
  /// `values` is where each parameter stands, `feed` is how loud the sound is at each of its
  /// frequencies, and `held` is the dot that is held down.
  fn scene(&mut self, values: &[f32], feed: &[f32], held: Option<usize>) -> Scene {
    let glass = self.glass;
    (self.spectrum).show(feed.to_vec(), |feed| spectrum(feed), glass, 0);
    let under = SPECTRUM_OBJECTS + self.grid.len();
    let drawn = |(values, held): &(Vec<f32>, Option<usize>)| curve(values, *held);
    (self.curve).show((values.to_vec(), held), drawn, glass, under);
    // the scene owns this frame's objects, so the ones that never change are copied into it
    let mut objects = self.fixed.clone();
    for shown in [&self.spectrum.objects, &self.grid, &self.curve.objects] {
      objects.extend(shown.iter().cloned());
    }
    let [red, green, blue, _] = CURVE;
    Scene {
      size: SIZE.into(),
      heights: [-8.0, 24.0],
      lean: Vec2::new(0.0, -0.25),
      // dimmed, so that what the screen shows stands out
      light: Light {
        color: Vec3::splat(0.6),
        ambient: Vec3::splat(0.2),
        ..Light::default()
      },
      look: Look::default(),
      objects,
      lamps: (self.screen)
        .lamps(SCREEN.x, SCREEN.y, Vec3::new(red, green, blue), 1.0)
        .collect(),
    }
  }
}

impl Skin for Equalizer {
  fn prepare(&mut self, gpu: &Gpu, view: &View, _over: &mut Flat) {
    self.dots = dots(view.values);
    let scene = self.scene(view.values, view.feed, view.held);
    (self.renderer).prepare(gpu.device, gpu.queue, &scene, view.pixels);
  }

  fn draw(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView, place: [u32; 4]) {
    let [x, y, width, height] = place.map(|side| side as f32);
    let on = Place {
      x,
      y,
      width,
      height,
    };
    self.renderer.render(encoder, target, on, place);
  }

  fn changed(&self) -> Option<&[[u32; 4]]> {
    self.renderer.changed()
  }

  fn target_at(&self, at: [f32; 2]) -> Option<Target> {
    // the dot drawn last lies over the others
    let on = |dot: &[f32; 2]| (at[0] - dot[0]).hypot(at[1] - dot[1]) <= DOT;
    self.dots.iter().rposition(on).map(Target::Control)
  }
}

/// Writes a made-up sound's loudness into `feed`.
///
/// A block at a time it writes how loud the sound is at each frequency. The sound is louder in the
/// bass, with peaks that drift, and the bands shape it as `values` has them. This is what a
/// plugin's audio thread does at the end of each block.
fn sound(feed: &Feed, values: &[AtomicU32; PARAMETERS]) {
  let mut blocks = 0.0_f32;
  let mut noise = 0x2545_f491_u32;
  let mut levels = [LOUDNESS.plain(0.0); BINS];
  loop {
    let bands = bands(
      &values
        .each_ref()
        .map(|value| f32::from_bits(value.load(Ordering::Relaxed))),
    );
    for (bin, level) in levels.iter_mut().enumerate() {
      let across = across(bin);
      let frequency = HERTZ.plain(across);
      // a xorshift step, whose top bits are spread from minus a half to a half
      noise ^= noise << 13;
      noise ^= noise >> 17;
      noise ^= noise << 5;
      let jitter = (noise >> 8) as f32 / (1 << 24) as f32 - 0.5;
      let drift = (blocks * 0.05 + across * 9.0).sin() * (blocks * 0.017 + across * 23.0).sin();
      let shaped: f32 = bands.iter().map(|band| band.gain_at(frequency, RATE)).sum();
      let loud = -30.0 - 14.0 * across + 7.0 * drift + 8.0 * jitter + shaped;
      // an analyzer follows the sound with a lag
      *level += (loud - *level) * 0.35;
      feed.set(bin, *level);
    }
    blocks += 1.0;
    // an audio thread is called back for each block, which this stands in for
    thread::sleep(BLOCK);
  }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut settings = Settings::new("potpot equalizer", SIZE);
  let feed = Feed::new(BINS);
  settings.feed = feed.clone();
  let (dials, controls) = wiring();
  // no host has these parameters: each stands where the panel last set it, kept where the
  // thread that makes the sound reads it too
  let values: Arc<[AtomicU32; PARAMETERS]> = Arc::new(std::array::from_fn(|parameter| {
    AtomicU32::new(dials[parameter].default.to_bits())
  }));
  let params = (dials.into_iter().enumerate())
    .map(|(parameter, dial)| {
      let (value, set) = (Arc::clone(&values), Arc::clone(&values));
      Param::new(
        dial,
        move || f32::from_bits(value[parameter].load(Ordering::Relaxed)),
        || {},
        move |to| set[parameter].store(to.to_bits(), Ordering::Relaxed),
        || {},
      )
    })
    .collect();
  // the thread is left running, and ends with the process when the window closes
  thread::spawn(move || sound(&feed, &values));
  let opened = potpot_window::open(settings, params, controls, Opening::alone(), Equalizer::new)?;
  opened.window.run_until_closed()?;
  Ok(())
}
