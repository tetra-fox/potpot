//! A parametric equalizer of four bands, opened by `just example equalizer`.
//!
//! `just render equalizer out.png` draws it to a file in place of the window, with a gain set on
//! each band.
//!
//! Each band has four parameters: its frequency, its gain, its Q and whether it is on. Five
//! controls turn them, and each shows where the others have put them:
//!
//! - a dot on the screen, dragged across for the frequency and up for the gain, with the wheel
//!   on the Q
//! - a knob on the frequency
//! - a fader on the gain
//! - a knob on the Q
//! - a button that turns the band on and off
//!
//! A control names the parameters it turns, and several controls name the same one. So a
//! dragged dot turns its band's knob and slides its fader, and the knob moves the dot. A double
//! click on a dot, a knob or a fader puts back what it turns, and a double click on a readout
//! types a value.
//!
//! The dots' parameters lie along the same [`Axis`] each as the curve is drawn along, so a dot
//! is at its band's frequency and gain in the picture. The screen draws the curve of the bands
//! that are on, over a spectrum that a thread standing in for a plugin's audio thread writes
//! through a [`Feed`]. That thread reads the same parameters, so the spectrum and the two
//! meters beside the screen rise and fall with the bands.

mod shared;

use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use std::time::Duration;

use potpot::glam::{Mat4, Vec2, Vec3};
use potpot::parts::{self, Button, Fader, Knob, Meter, Screen};
use potpot::{Light, Look, Material, Mesh, Object, Renderer, Scene};
use potpot_window::controls::axis::Axis;
use potpot_window::controls::flat::{Color, Flat};
use potpot_window::controls::interaction::{Control, Dial, Drag, Push, Readout, Target, Touch};
use potpot_window::controls::layout::{self, Pack};
use potpot_window::controls::response::{self, Band, Kind};
use potpot_window::controls::stroke;
use potpot_window::{Feed, Gpu, Settings, Skin, View, wgpu};

use crate::shared::{ALUMINUM, BLACK, CYAN, Drawn, LABELS, Noise, Picture, Plot, SEAT};

const SIZE: [f32; 2] = [840.0, 520.0];
/// The middle of the screen.
const SCREEN: Vec2 = Vec2::new(384.0, 162.0);
/// The width and length of the screen's face.
const GLASS: Vec2 = Vec2::new(700.0, 230.0);
/// The distance from the edge of the face in to the picture.
const INSET: f32 = 12.0;

/// A band as it starts out.
struct Start {
  name: &'static str,
  kind: Kind,
  /// The frequency in hertz.
  frequency: f32,
  q: f32,
  /// The color of its dot and of its button's light.
  color: Vec3,
}

/// The bands, from the lowest.
const BANDS: [Start; 4] = [
  Start {
    name: "LOW",
    kind: Kind::LowShelf,
    frequency: 90.0,
    q: 0.7,
    color: Vec3::new(1.0, 0.45, 0.1),
  },
  Start {
    name: "LOW MID",
    kind: Kind::Bell,
    frequency: 400.0,
    q: 1.0,
    color: Vec3::new(1.0, 0.85, 0.15),
  },
  Start {
    name: "HIGH MID",
    kind: Kind::Bell,
    frequency: 2400.0,
    q: 1.0,
    color: Vec3::new(0.4, 1.0, 0.35),
  },
  Start {
    name: "HIGH",
    kind: Kind::HighShelf,
    frequency: 9000.0,
    q: 0.7,
    color: Vec3::new(0.75, 0.5, 1.0),
  },
];

// a band's parameters, counted from its first. the panel's parameters are each band's in turn
const FREQUENCY: usize = 0;
const GAIN: usize = 1;
const Q: usize = 2;
const ON: usize = 3;
const PARAMETERS_EACH: usize = 4;
const PARAMETERS: usize = BANDS.len() * PARAMETERS_EACH;
// a band's controls, counted from its first. the panel's controls are each band's in turn
const DOT: usize = 0;
const FREQUENCY_KNOB: usize = 1;
const FADER: usize = 2;
const Q_KNOB: usize = 3;
const BUTTON: usize = 4;
const CONTROLS_EACH: usize = 5;

// what the picture is drawn along and the parameters lie along: hertz across and decibels up,
// and the run of a band's q
const HERTZ: Axis = Axis::log([20.0, 20000.0]);
const DECIBELS: Axis = Axis::linear([-18.0, 18.0]);
const QS: Axis = Axis::log([0.3, 8.0]);
/// The decibels the spectrum shows, from its foot to its top.
const LOUDNESS: Axis = Axis::linear([-60.0, 0.0]);
/// The decibels the meters show, from no lights to all.
const METERED: Axis = Axis::linear([-48.0, -20.0]);
/// The sample rate the curve is drawn for, in hertz.
const RATE: f32 = 48000.0;
/// The count of the feed's slots that are the spectrum.
///
/// Each is how loud the sound is at one frequency, in decibels. After them comes how loud
/// each side is, in decibels too.
const BINS: usize = 64;
// the made-up sound, in decibels: how loud its lowest frequency is, how much quieter its
// highest is, and how far its drifting peaks and its noise go either way
const BASS: f32 = -30.0;
const TILT: f32 = 14.0;
const DRIFT: f32 = 7.0;
const JITTER: f32 = 8.0;
/// The time between two blocks of the made-up sound.
const BLOCK: Duration = Duration::from_millis(30);
/// The count of blocks of the sound that a picture is drawn after.
const PICTURED_AFTER: u32 = 40;
/// Each band's gain in a picture, in decibels.
const PICTURED_GAINS: [f32; BANDS.len()] = [5.0, -7.0, 4.0, -3.0];

// a band's strip under the screen: how wide it is, how far down its row of controls and what
// is printed under them are, and the room each of its controls has across the panel
const STRIP: f32 = 200.0;
const TITLE_Y: f32 = 312.0;
const CONTROLS_Y: f32 = 392.0;
const NAME_Y: f32 = 456.0;
const READOUT_Y: f32 = 482.0;
const ASIDE: f32 = 66.0;
// the line cut in the panel between two strips: its length and how far down its middle is
const DIVIDER: f32 = 196.0;
const DIVIDER_Y: f32 = 398.0;
const BUTTON_SIZE: f32 = 18.0;
const TITLE_LETTERS: f32 = 7.5;
/// How far the fader slides.
const TRAVEL: f32 = 84.0;
/// Half a readout's width and height.
const READOUT: Vec2 = Vec2::new(28.0, 9.0);
const LETTERS: f32 = 6.5;
// the two meters: how far across their middle is, the room each has across the panel, and how
// far down their foot is
const METERS: usize = 2;
const METERS_X: f32 = 785.0;
const METER_SLOT: f32 = 18.0;
const METER_FOOT: f32 = SCREEN.y + GLASS.y / 2.0;

/// How far from its middle a band's dot reaches.
const DOT_REACH: f32 = 6.0;
const SPECTRUM: Color = [0.04, 0.2, 0.24, 1.0];
/// The count of objects that show the spectrum.
///
/// The spectrum is in one color.
const SPECTRUM_OBJECTS: usize = 1;

/// Returns the picture's place on the screen.
fn plot() -> Plot {
  Plot::of(&Screen::new(GLASS.x, GLASS.y), SCREEN, INSET)
}

/// Where a band's parts are across the panel.
struct Strip {
  /// The strip's left end and right end.
  span: [f32; 2],
  /// The middles of the frequency knob, fader and Q knob.
  controls: [f32; 3],
  /// The middle of the button.
  button: f32,
  /// The middle of the band's name, beside the button.
  name: f32,
}

/// Returns where each band's parts are across the panel.
///
/// The strips are side by side in the middle of the panel, and a band's button and name are
/// side by side in the middle of its strip.
fn strips() -> [Strip; BANDS.len()] {
  let spans = layout::pack([0.0, SIZE[0]], &[STRIP; BANDS.len()], Pack::Center);
  std::array::from_fn(|band| {
    let span = spans[band];
    let name = stroke::width(BANDS[band].name.chars().count(), TITLE_LETTERS);
    let title = layout::pack(span, &[BUTTON_SIZE, 10.0, name], Pack::Center);
    Strip {
      span,
      controls: shared::centered(span, ASIDE),
      button: layout::middle(title[0]),
      name: layout::middle(title[2]),
    }
  })
}

/// Returns whether a band is on.
fn is_on(values: &[f32], band: usize) -> bool {
  values[band * PARAMETERS_EACH + ON] > 0.5
}

/// Returns each band as the parameters have it.
///
/// A band that is off is `None`.
///
/// `values` is where each parameter stands, as a part of its whole turn.
fn bands(values: &[f32]) -> [Option<Band>; BANDS.len()] {
  std::array::from_fn(|band| {
    let value = |parameter: usize| values[band * PARAMETERS_EACH + parameter];
    is_on(values, band).then(|| Band {
      kind: BANDS[band].kind,
      frequency: HERTZ.plain(value(FREQUENCY)),
      gain: DECIBELS.plain(value(GAIN)),
      q: QS.plain(value(Q)),
    })
  })
}

/// Returns every parameter and every control.
fn wiring() -> (Vec<Dial>, Vec<Control>) {
  let dials = (BANDS.iter())
    .flat_map(|band| {
      [
        Dial::along(HERTZ, band.frequency, None, 0, " HZ"),
        Dial::along(DECIBELS, 0.0, None, 1, " DB"),
        Dial::along(QS, band.q, None, 2, ""),
        Dial::linear([0.0, 1.0], 1.0, Some(1.0), 0, ""),
      ]
    })
    .collect();
  let readout = Readout::new(8);
  let controls = (0..BANDS.len())
    .flat_map(|band| {
      let first = band * PARAMETERS_EACH;
      // dragged over the whole picture, so the dot keeps to the pointer
      let mut dot = Control::pad(
        first + FREQUENCY,
        first + GAIN,
        plot().size().into(),
        readout,
      );
      dot.wheel = Some(first + Q);
      // the cap goes as far up the panel as the pointer does
      let mut fader = Control::new(first + GAIN, readout);
      fader.feel.drag = Drag::Along {
        way: [0.0, -1.0],
        turn: 1.0 / TRAVEL,
      };
      let mut button = Control::new(first + ON, readout);
      button.touch = Touch::Press(Push::Latching);
      // in the order of `DOT` to `BUTTON`
      [
        dot,
        Control::new(first + FREQUENCY, readout),
        fader,
        Control::new(first + Q, readout),
        button,
      ]
    })
    .collect();
  (dials, controls)
}

/// Returns how far across the picture a slot of the feed is.
fn bin_place(bin: usize) -> f32 {
  bin as f32 / (BINS - 1) as f32
}

/// Returns the spectrum as a filled region.
///
/// The region lies under how loud each frequency is. `bins` is how loud the sound is at each
/// of its frequencies, in decibels.
fn spectrum(bins: &[f32]) -> Flat {
  let (mut flat, plot) = (Flat::default(), plot());
  let edge: Vec<[f32; 2]> = (bins.iter().enumerate())
    .map(|(bin, loud)| plot.at([bin_place(bin), LOUDNESS.turn(*loud)]))
    .collect();
  flat.area(&edge, plot.max.y, SPECTRUM);
  flat
}

/// Returns the lines and lettering that divide the picture.
///
/// There is a line for each tenfold step of frequency and for each six decibels.
fn grid() -> Flat {
  let (mut flat, plot) = (Flat::default(), plot());
  let hertz = [(100.0, "100"), (1000.0, "1K"), (10000.0, "10K")];
  for (frequency, _) in hertz {
    let place = HERTZ.turn(frequency);
    flat.line(
      plot.at([place, 0.0]),
      plot.at([place, 1.0]),
      1.0,
      shared::GRID,
    );
  }
  let decibels = [
    (-12.0, "-12"),
    (-6.0, ""),
    (0.0, "0"),
    (6.0, ""),
    (12.0, "+12"),
  ];
  for (gain, _) in decibels {
    let up = DECIBELS.turn(gain);
    flat.line(plot.at([0.0, up]), plot.at([1.0, up]), 1.0, shared::GRID);
  }
  // after every line, so that the two colors are two objects on the screen
  for (frequency, name) in hertz {
    let [x, y] = plot.at([HERTZ.turn(frequency), 0.0]);
    flat.legend(name, [x + 4.0, y - 10.0], 6.0, 1.0, LABELS);
  }
  for (gain, name) in decibels {
    let [x, y] = plot.at([1.0, DECIBELS.turn(gain)]);
    // at the right, clear of the sheen in the glass's top left corner
    let left = x - 2.0 - stroke::width(name.chars().count(), 6.0);
    flat.legend(name, [left, y - 9.0], 6.0, 1.0, LABELS);
  }
  flat
}

/// Returns where each band's dot is.
///
/// A dot's two parameters lie along the picture's own axes, so they are its place in the
/// picture: across at its band's frequency and up at its band's gain.
fn dots(values: &[f32]) -> [[f32; 2]; BANDS.len()] {
  std::array::from_fn(|band| {
    let first = band * PARAMETERS_EACH;
    plot().at([values[first + FREQUENCY], values[first + GAIN]])
  })
}

/// Returns the curve of the bands that are on, and the dots.
///
/// There is a dot for each band, on or off.
///
/// `values` is where each parameter stands and `held` is the band one of whose controls is held
/// down. That band's dot is filled, and where the band stands is written out at the top right
/// of the picture, or at its foot while the curve is up there.
fn curve(values: &[f32], held: Option<usize>) -> Flat {
  let (mut flat, plot) = (Flat::default(), plot());
  let sounding: Vec<Band> = bands(values).into_iter().flatten().collect();
  let points: Vec<[f32; 2]> = response::trace(&sounding, RATE, HERTZ, DECIBELS, 280)
    .into_iter()
    .map(|point| plot.at(point))
    .collect();
  flat.curve(&points, 2.2, CYAN);
  if let Some(band) = held {
    let value = |parameter: usize| values[band * PARAMETERS_EACH + parameter];
    let stands = format!(
      "{}  {:.0} HZ  {:+.1} DB  Q {:.2}",
      BANDS[band].name,
      HERTZ.plain(value(FREQUENCY)),
      DECIBELS.plain(value(GAIN)),
      QS.plain(value(Q)),
    );
    let [red, green, blue] = BANDS[band].color.to_array();
    let [x, y] = plot.at([1.0, 1.0]);
    let left = x - 6.0 - stroke::width(stands.chars().count(), 7.0);
    // at the top right, or over the names of the frequencies at the foot while the curve is
    // up where the top one would be
    let crossed = (points.iter()).any(|point| point[0] > left - 6.0 && point[1] < y + 22.0);
    let top = if crossed { plot.max.y - 30.0 } else { y + 6.0 };
    flat.legend(&stands, [left, top], 7.0, 1.2, [red, green, blue, 1.0]);
  }
  for (band, dot) in dots(values).into_iter().enumerate() {
    let on = is_on(values, band);
    // a band that is off keeps its dot, dimmed
    let [red, green, blue] = (BANDS[band].color * if on { 1.0 } else { 0.3 }).to_array();
    shared::dot(
      &mut flat,
      dot,
      DOT_REACH,
      [red, green, blue, 1.0],
      held == Some(band),
    );
  }
  flat
}

/// The panel as potpot draws it.
///
/// It holds its meshes, each made once.
struct Equalizer {
  /// The renderer, which also finds the pointer on the scene.
  renderer: Renderer,
  /// What each of the scene's objects is, in the same order.
  targets: Vec<Option<Target>>,
  /// Everything that never moves or changes.
  fixed: Drawn,
  screen: Screen,
  glass: Material,
  /// The lines and lettering on the screen, over the spectrum.
  grid: Vec<Object>,
  /// The spectrum, drawn from how loud each frequency is.
  spectrum: Picture<Vec<f32>>,
  /// The curve and the dots.
  ///
  /// They are drawn from the parameters and the band that is held.
  curve: Picture<(Vec<f32>, Option<usize>)>,
  /// Where each band's dot was last drawn.
  dots: [[f32; 2]; BANDS.len()],
  /// A knob's body and the line painted on it.
  knob: [Arc<Mesh>; 2],
  /// A fader's cap and its stripe.
  cap: [Arc<Mesh>; 2],
  button: Button,
  button_body: Arc<Mesh>,
  meter: Meter,
  segment: Arc<Mesh>,
  /// Each readout's control and its middle.
  readouts: Vec<(usize, Vec2)>,
  strips: [Strip; BANDS.len()],
  /// How far across each meter's middle is.
  meter_xs: [f32; METERS],
}

impl Equalizer {
  fn new(gpu: &Gpu) -> Self {
    let size = Vec2::from(SIZE);
    let ink = Material::matte(Vec3::splat(0.8));
    let hole = Material::matte(SEAT);
    let mut fixed = Drawn::default();
    fixed.put(
      parts::plate(size.x, size.y, 6.0),
      Vec2::ZERO,
      Material::textured(Vec3::new(0.075, 0.08, 0.09)),
    );
    fixed.letters("EQUALIZER", Vec2::new(SCREEN.x, 22.0), 9.0, ink);
    fixed.screws(size, 16.0, 6.0);

    let screen = Screen::new(GLASS.x, GLASS.y);
    let glass = shared::glass(&screen);
    fixed.put(screen.face(), SCREEN, glass);
    fixed.put(screen.bezel(), SCREEN, Material::glossy(BLACK));

    // the gaps the meters' lights sit in, with what they show over them
    let meter = Meter::ladder(18, 11.0, GLASS.y);
    let seat = Arc::new(meter.seat());
    let half = METER_SLOT * METERS as f32 / 2.0;
    let meter_xs = shared::centered([METERS_X - half, METERS_X + half], METER_SLOT);
    for x in meter_xs {
      fixed.extend(meter.seats(&seat, x, METER_FOOT));
    }
    fixed.letters("OUT", Vec2::new(METERS_X, 30.0), 6.5, ink);

    // each band's strip: its name beside its button, the scales of its knobs and its fader,
    // what each is, and a readout under each
    let knob = Knob::knurled(22.0, 22.0);
    let scale = Arc::new(parts::ticks(
      11,
      parts::KNOB_SWEEP,
      knob.radius + 5.0,
      knob.radius + 9.0,
      5,
    ));
    let button = Button::round(BUTTON_SIZE, 7.0);
    let mut readouts = Vec::new();
    let strips = strips();
    for (band, (start, strip)) in BANDS.iter().zip(&strips).enumerate() {
      let [_, fader, _] = strip.controls.map(|x| Vec2::new(x, CONTROLS_Y));
      let first = band * CONTROLS_EACH;
      fixed.put(button.seat(2.0), Vec2::new(strip.button, TITLE_Y), hole);
      let name = Vec2::new(strip.name, TITLE_Y);
      fixed.letters(start.name, name, TITLE_LETTERS, ink);
      fixed.put(parts::fader_slot(TRAVEL, 5.0), fader, hole);
      fixed.put(parts::fader_ticks(9, TRAVEL, 15.0, 20.0, 4), fader, ink);
      let under = [(FREQUENCY_KNOB, "FREQ"), (FADER, "GAIN"), (Q_KNOB, "Q")];
      for ((control, name), x) in under.into_iter().zip(strip.controls) {
        if control != FADER {
          let on_panel = Mat4::from_translation(Vec3::new(x, CONTROLS_Y, 0.0));
          fixed.place(&scale, on_panel, ink, None);
        }
        fixed.letters(name, Vec2::new(x, NAME_Y), 6.5, ink);
        let window = Vec2::new(x, READOUT_Y);
        fixed.readout(window, READOUT, first + control);
        readouts.push((first + control, window));
      }
      // a line cut in the panel between this strip and the next
      if band > 0 {
        let half = Vec3::new(0.7, DIVIDER / 2.0, 0.0);
        let line = Mesh::cuboid(-half, half + Vec3::Z * 0.2);
        fixed.put(line, Vec2::new(strip.span[0], DIVIDER_Y), hole);
      }
    }

    let cap = Fader::console(22.0, 32.0, 13.0);
    Self {
      renderer: Renderer::new(gpu.device, gpu.format),
      targets: Vec::new(),
      fixed,
      screen,
      glass,
      grid: parts::picture(&grid(), SCREEN.x, SCREEN.y, glass, SPECTRUM_OBJECTS),
      spectrum: Picture::new(),
      curve: Picture::new(),
      dots: [[0.0; 2]; BANDS.len()],
      knob: [knob.body(), knob.line()].map(Arc::new),
      cap: [cap.body(), cap.line()].map(Arc::new),
      button_body: Arc::new(button.body()),
      button,
      segment: Arc::new(meter.segment()),
      meter,
      readouts,
      strips,
      meter_xs,
    }
  }

  /// Returns the scene and what each of its objects is.
  ///
  /// `values` is where each parameter stands, `feed` is the spectrum and then how loud
  /// each side is, and `held` is the control that is held down.
  fn scene(
    &mut self,
    values: &[f32],
    feed: &[f32],
    held: Option<usize>,
  ) -> (Scene, Vec<Option<Target>>) {
    // the scene owns this frame's objects, so the ones that never move are copied into it
    let mut drawn = self.fixed.clone();
    let [red, green, blue, _] = CYAN;
    let lit = Vec3::new(red, green, blue);
    let mut lamps: Vec<_> = (self.screen).lamps(SCREEN.x, SCREEN.y, lit, 1.0).collect();

    let metal = [Material::spun(ALUMINUM), Material::matte(BLACK)];
    let rubber = Material::silicone(Vec3::splat(0.62));
    for (band, (start, strip)) in BANDS.iter().zip(&self.strips).enumerate() {
      let first = band * CONTROLS_EACH;
      let value = |parameter: usize| values[band * PARAMETERS_EACH + parameter];
      let [frequency, fader, q] = strip.controls.map(|x| Vec2::new(x, CONTROLS_Y));
      let knobs = [(FREQUENCY_KNOB, FREQUENCY, frequency), (Q_KNOB, Q, q)];
      for (control, parameter, at) in knobs {
        let turn = value(parameter);
        drawn.knob(&self.knob, metal, at, turn, first + control);
      }
      drawn.fader(&self.cap, fader, TRAVEL, value(GAIN), first + FADER);

      // the button is lit in its band's color and most of the way in while the band is on
      let on = is_on(values, band);
      let level = f32::from(u8::from(on));
      let at = Vec2::new(strip.button, TITLE_Y);
      let depth = Push::Latching.depth(on, held == Some(first + BUTTON));
      drawn.place(
        &self.button_body,
        self.button.transform(at.x, at.y, depth),
        rubber.lit(start.color, level, self.button.glow_radius()),
        Some(Target::Control(first + BUTTON)),
      );
      if on {
        lamps.push(self.button.lamp(at.x, at.y, start.color, 0.6));
      }
    }

    for (x, loud) in self.meter_xs.into_iter().zip(&feed[BINS..]) {
      let level = METERED.turn(*loud);
      drawn.extend((self.meter).lights(&self.segment, x, METER_FOOT, level, None));
      lamps.extend(self.meter.lamp(x, METER_FOOT, level));
    }

    // the screen's three pictures, each made again only when what it shows is new
    let glass = self.glass;
    let bins = feed[..BINS].to_vec();
    (self.spectrum).show(bins, |bins| spectrum(bins), SCREEN, glass, 0);
    let under = SPECTRUM_OBJECTS + self.grid.len();
    let band = held.map(|control| control / CONTROLS_EACH);
    let draw = |(values, band): &(Vec<f32>, Option<usize>)| curve(values, *band);
    (self.curve).show((values.to_vec(), band), draw, SCREEN, glass, under);
    for picture in [&self.spectrum.objects, &self.grid, &self.curve.objects] {
      drawn.extend(picture.iter().cloned());
    }

    let scene = Scene {
      size: SIZE.into(),
      heights: [-8.0, 40.0],
      lean: Vec2::new(0.0, -0.25),
      // dimmed, so that what the screen shows stands out
      light: Light {
        color: Vec3::splat(0.6),
        ambient: Vec3::splat(0.2),
        ..Light::default()
      },
      look: Look::default(),
      objects: drawn.objects,
      lamps,
    };
    (scene, drawn.targets)
  }
}

impl Skin for Equalizer {
  fn prepare(&mut self, gpu: &Gpu, view: &View, over: &mut Flat) {
    self.dots = dots(view.values);
    let (scene, targets) = self.scene(view.values, view.feed, view.held);
    (self.renderer).prepare(gpu.device, gpu.queue, &scene, view.pixels);
    self.targets = targets;
    for &(control, middle) in &self.readouts {
      shared::written(over, view, control, middle, LETTERS);
    }
  }

  fn draw(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView, place: [u32; 4]) {
    shared::render(&self.renderer, encoder, target, place);
  }

  fn changed(&self) -> Option<&[[u32; 4]]> {
    self.renderer.changed()
  }

  fn target_at(&self, at: [f32; 2]) -> Option<Target> {
    // a dot lies flat on the screen, so it is found by its place and the rest by the scene
    if let Some(band) = shared::dot_at(&self.dots, at, DOT_REACH) {
      return Some(Target::Control(band * CONTROLS_EACH + DOT));
    }
    self.targets[self.renderer.pick(at.into())?]
  }
}

/// Returns the function that writes one block of the sound.
///
/// It writes how loud a made-up sound is at each frequency and then on each side into `feed`, as
/// the bands shape it. The sound is louder in the bass, with peaks that drift, and each band that
/// is on adds its gain as `values` has it. This is what a plugin's audio thread does at the end
/// of each block.
fn sound(feed: Feed, values: Arc<[AtomicU32]>) -> impl FnMut() + Send {
  let mut blocks = 0.0_f32;
  let mut noise = Noise::new();
  let mut levels = [LOUDNESS.plain(0.0); BINS];
  move || {
    let stands: [f32; PARAMETERS] =
      std::array::from_fn(|parameter| shared::value(&values, parameter));
    let bands = bands(&stands);
    let mut power = 0.0;
    for (bin, level) in levels.iter_mut().enumerate() {
      let place = bin_place(bin);
      let frequency = HERTZ.plain(place);
      let drift = (blocks * 0.05 + place * 9.0).sin() * (blocks * 0.017 + place * 23.0).sin();
      let shaped: f32 = (bands.iter().flatten())
        .map(|band| band.gain_at(frequency, RATE))
        .sum();
      let made_up = BASS - TILT * place + DRIFT * drift + JITTER * noise.sample();
      let loud = made_up + shaped;
      // an analyzer follows the sound with a lag
      *level += (loud - *level) * 0.35;
      feed.set(bin, *level);
      power += 10.0_f32.powf(*level / 10.0);
    }
    // the two sides are as loud as the mean power of the frequencies, each a little off the
    // other
    let loud = 10.0 * (power / BINS as f32).log10();
    for side in 0..METERS {
      feed.set(BINS + side, loud + 2.0 * noise.sample());
    }
    blocks += 1.0;
  }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut settings = Settings::new("potpot equalizer", SIZE);
  let feed = Feed::new(BINS + METERS);
  settings.feed = feed.clone();
  let (dials, controls) = wiring();
  let (params, values) = shared::params(dials);
  if shared::window::file().is_some() {
    for (band, gain) in PICTURED_GAINS.into_iter().enumerate() {
      shared::set(&values, band * PARAMETERS_EACH + GAIN, DECIBELS.turn(gain));
    }
  }
  let sound = shared::window::Sound {
    block: Box::new(sound(feed, values)),
    every: BLOCK,
    pictured_after: PICTURED_AFTER,
  };
  shared::window::run(settings, params, controls, Some(sound), Equalizer::new)
}
