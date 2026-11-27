//! A compressor, opened by `just example compressor`.
//!
//! `just render compressor out.png` draws it to a file in place of the window.
//!
//! Five knobs turn the threshold, the ratio, the attack, the release and the makeup gain. The
//! screen on the left draws what they make of a level: the level in across and the level out up,
//! bent at the threshold. The dot on the bend is a sixth control on the same parameters as the
//! first two knobs. A drag along the slope of the curve under the bend moves the threshold and
//! the wheel over the dot turns the ratio, and either way the knobs turn with it.
//!
//! A thread standing in for a plugin's audio thread runs a made-up drum loop through a
//! compressor that reads the same parameters, a sample at a time. Through a [`Feed`] it hands
//! over what the rest of the panel shows:
//!
//! - the levels in and out and the gain reduction, on the three meters
//! - the last few seconds of the level in and the reduction, on the wide screen
//! - the small dot that rides the curve at the level of the sound

mod shared;

use std::f32::consts::{PI, TAU};
use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use std::time::Duration;

use potpot::glam::{Mat4, Vec2, Vec3};
use potpot::parts::{self, Knob, Meter, Screen};
use potpot::{Light, Look, Material, Mesh, Object, Renderer, Scene};
use potpot_window::controls::axis::Axis;
use potpot_window::controls::flat::{Color, Flat};
use potpot_window::controls::interaction::{Control, Dial, Drag, Readout, Target};
use potpot_window::controls::layout;
use potpot_window::{Feed, Gpu, Settings, Skin, View, wgpu};

use crate::shared::{BLACK, Drawn, INK, LABELS, Noise, Picture, Plot};

const SIZE: [f32; 2] = [760.0, 470.0];

// the parameters, each with a knob at the same place among the controls. after the knobs comes
// the dot on the bend of the curve
const THRESHOLD: usize = 0;
const RATIO: usize = 1;
const ATTACK: usize = 2;
const RELEASE: usize = 3;
const MAKEUP: usize = 4;
const KNOBS: usize = 5;
const BEND: usize = KNOBS;

/// The quietest level there is, in decibels.
///
/// Levels are counted from the loudest that goes in, which is zero.
const QUIETEST: f32 = -60.0;
/// The most decibels the makeup adds.
const MOST_MAKEUP: f32 = 24.0;
// what the parameters lie along. the threshold is a level in, and the screens are drawn along
// the same axis
const LEVELS: Axis = Axis::linear([QUIETEST, 0.0]);
const RATIOS: Axis = Axis::log([1.0, 20.0]);
const ATTACKS: Axis = Axis::log([0.1, 100.0]);
const RELEASES: Axis = Axis::log([10.0, 1000.0]);
const MAKEUPS: Axis = Axis::linear([0.0, MOST_MAKEUP]);
/// The levels that come out, which the curve is drawn up along.
///
/// The loudest is the loudest that goes in with all the makeup, so the curve and its bend are
/// in the picture wherever the parameters stand.
const LEVELS_OUT: Axis = Axis::linear([QUIETEST, MOST_MAKEUP]);
/// The decibels of reduction that light all of its meter.
const REDUCTIONS: Axis = Axis::linear([0.0, 24.0]);
/// The decibels the level meters show, from no light to all.
const METERED: Axis = Axis::linear([-42.0, 0.0]);

// the slots of the feed: the level in, the level out and the reduction, in decibels, then the
// level in over the last blocks, oldest first, and then the reduction over the same blocks
const IN: usize = 0;
const OUT: usize = 1;
const REDUCTION: usize = 2;
const PAST: usize = 3;
/// The count of blocks the wide screen looks back over.
const BLOCKS: usize = 110;
/// The sample rate of the made-up sound, in hertz.
const RATE: f32 = 48000.0;
/// The time between two blocks of it.
const BLOCK: Duration = Duration::from_millis(30);
/// The count of blocks of the sound that a picture is drawn after.
const PICTURED_AFTER: u32 = 150;

// the middles of the two screens and the widths and lengths of their faces
const CURVE: Vec2 = Vec2::new(146.0, 160.0);
const CURVE_GLASS: Vec2 = Vec2::new(220.0, 220.0);
const PAST_SCREEN: Vec2 = Vec2::new(455.0, 160.0);
const PAST_GLASS: Vec2 = Vec2::new(330.0, 220.0);
/// The distance from the edge of a face in to its picture.
const INSET: f32 = 12.0;
// what the three meters show and their names, the room each has across the panel, and how far
// down their foot and their top are
const METERS: [(usize, &str); 3] = [(IN, "IN"), (REDUCTION, "GR"), (OUT, "OUT")];
const METER_SLOT: f32 = 28.0;
const METER_FOOT: f32 = CURVE.y + CURVE_GLASS.y / 2.0;
const METER_TOP: f32 = CURVE.y - CURVE_GLASS.y / 2.0;

/// Each knob's name, in the order of the parameters.
const KNOB_NAMES: [&str; KNOBS] = ["THRESHOLD", "RATIO", "ATTACK", "RELEASE", "MAKEUP"];
/// The room across the panel for each knob of a pair.
const KNOB_SLOT: f32 = 110.0;
// how far down the knobs' middles, their names and their readouts are
const KNOB_Y: f32 = 346.0;
const NAME_Y: f32 = 400.0;
const READOUT_Y: f32 = 426.0;
/// Half a readout's width and height.
const READOUT: Vec2 = Vec2::new(32.0, 10.0);
const LETTERS: f32 = 7.0;

/// How far from its middle the dot on the bend reaches.
const DOT_REACH: f32 = 6.0;
/// The color of the knobs.
const CREAM: Vec3 = Vec3::new(0.74, 0.72, 0.66);
const TRACE: Color = [0.5, 1.0, 0.6, 1.0];
// the level in on the wide screen: its filled region and the line along its top
const LOUD: Color = [0.04, 0.17, 0.2, 1.0];
const LOUD_EDGE: Color = [0.2, 0.62, 0.7, 1.0];
/// The count of objects that show the level in.
///
/// There is one for each of its two colors.
const LOUD_OBJECTS: usize = 2;
/// How far down the names printed under the screens are.
const CAPTION_Y: f32 = 294.0;

/// Returns the place of the curve's picture on its screen.
fn curve_plot() -> Plot {
  Plot::of(&Screen::new(CURVE_GLASS.x, CURVE_GLASS.y), CURVE, INSET)
}

/// Returns the place of the wide screen's picture.
fn past_plot() -> Plot {
  Plot::of(&Screen::new(PAST_GLASS.x, PAST_GLASS.y), PAST_SCREEN, INSET)
}

/// Returns how many decibels a compressor takes off a level.
///
/// What a level is over the `threshold` by comes out as one part in `ratio` of it. All three
/// are in decibels but the ratio.
fn reduction(level: f32, threshold: f32, ratio: f32) -> f32 {
  ((level - threshold) * (1.0 - 1.0 / ratio)).max(0.0)
}

/// Returns every parameter and every control.
fn wiring() -> (Vec<Dial>, Vec<Control>) {
  // in the order of the parameters
  let dials = vec![
    Dial::along(LEVELS, -24.0, None, 1, " DB"),
    Dial::along(RATIOS, 4.0, None, 1, ":1"),
    Dial::along(ATTACKS, 5.0, None, 1, " MS"),
    Dial::along(RELEASES, 120.0, None, 0, " MS"),
    Dial::along(MAKEUPS, 0.0, None, 1, " DB"),
  ];
  let readout = Readout::new(8);
  let mut controls: Vec<Control> = (0..KNOBS)
    .map(|parameter| Control::new(parameter, readout))
    .collect();
  // as the threshold turns, the bend goes along a line that rises a decibel out for each
  // decibel in, which the makeup lifts whole. dragged along the way of that line the dot keeps
  // to the pointer, over the length of the line for the whole turn
  let [from, to] = [0.0, 1.0].map(|threshold| Vec2::from(bend(&[threshold, 0.0, 0.0, 0.0, 0.0])));
  let line = to - from;
  let mut bend = Control::new(THRESHOLD, readout);
  bend.feel.drag = Drag::Along {
    way: line.normalize().into(),
    turn: 1.0 / line.length(),
  };
  bend.wheel = Some(RATIO);
  controls.push(bend);
  (dials, controls)
}

/// Returns the level that comes out for a level that goes in.
///
/// Both are in decibels. `values` is where each parameter stands, as a part of its whole turn.
fn level_out(level: f32, values: &[f32]) -> f32 {
  let threshold = LEVELS.plain(values[THRESHOLD]);
  let ratio = RATIOS.plain(values[RATIO]);
  level - reduction(level, threshold, ratio) + MAKEUPS.plain(values[MAKEUP])
}

/// Returns where on the curve's picture a level in is.
fn on_curve(level: f32, values: &[f32]) -> [f32; 2] {
  curve_plot().at([
    LEVELS.turn(level),
    LEVELS_OUT.turn(level_out(level, values)),
  ])
}

/// Returns where the dot on the bend is.
fn bend(values: &[f32]) -> [f32; 2] {
  on_curve(LEVELS.plain(values[THRESHOLD]), values)
}

/// Returns the lines that divide the curve's picture.
///
/// There is one for each twelve decibels, and one where the level out is the level in.
fn curve_grid() -> Flat {
  let (mut flat, plot) = (Flat::default(), curve_plot());
  plot.divide(&mut flat, 5, 7);
  let same = |level: f32| plot.at([LEVELS.turn(level), LEVELS_OUT.turn(level)]);
  flat.line(
    same(LEVELS.plain(0.0)),
    same(LEVELS.plain(1.0)),
    1.0,
    shared::GRID,
  );
  // the loudest that goes in, as a level out, named at the left, where the curve is under it
  // whatever the makeup
  let [x, y] = plot.at([0.0, LEVELS_OUT.turn(LEVELS.plain(1.0))]);
  flat.legend("0 DB", [x + 4.0, y + 4.0], 6.0, 1.0, LABELS);
  // over the curve's left end wherever the makeup puts it
  let [x, y] = plot.at([0.0, 0.42]);
  flat.legend("OUT", [x + 4.0, y], 6.0, 1.0, LABELS);
  let [x, y] = plot.at([1.0, 0.0]);
  flat.legend("IN", [x - 16.0, y - 10.0], 6.0, 1.0, LABELS);
  flat
}

/// Returns the curve of the level out over the level in.
///
/// The dot is on its bend.
///
/// `held` is whether the dot is held down.
fn curve(values: &[f32], held: bool) -> Flat {
  let mut flat = Flat::default();
  // straight to the bend and straight on from it
  let levels = [
    LEVELS.plain(0.0),
    LEVELS.plain(values[THRESHOLD]),
    LEVELS.plain(1.0),
  ];
  flat.curve(&levels.map(|level| on_curve(level, values)), 2.2, TRACE);
  shared::dot(&mut flat, bend(values), DOT_REACH, INK, held);
  flat
}

/// Returns the small dot that rides the curve.
///
/// `at` is where on the picture it is.
fn riding(at: [f32; 2]) -> Flat {
  let mut flat = Flat::default();
  flat.line(at, at, 5.0, [1.0, 1.0, 1.0, 1.0]);
  flat
}

/// Returns the lines that divide the wide screen's picture.
///
/// There is one for each twelve decibels up.
fn past_grid() -> Flat {
  let (mut flat, plot) = (Flat::default(), past_plot());
  plot.divide(&mut flat, 6, 5);
  flat
}

/// Returns the level in over the last blocks.
///
/// It is a filled region that stands on the foot of the picture, with a line along its top.
fn past_level(levels: &[f32]) -> Flat {
  let (mut flat, plot) = (Flat::default(), past_plot());
  let last = (levels.len() - 1) as f32;
  let edge: Vec<[f32; 2]> = (levels.iter().enumerate())
    .map(|(block, level)| plot.at([block as f32 / last, LEVELS.turn(*level)]))
    .collect();
  flat.area(&edge, plot.max.y, LOUD);
  flat.curve(&edge, 1.2, LOUD_EDGE);
  flat
}

/// Returns the reduction over the last blocks.
///
/// It is a line that hangs from the top of the picture, drawn with the threshold's line.
///
/// A decibel of reduction is as long as a decibel of level. `threshold` is where its parameter
/// stands. The threshold's name is under its line, or over it where the line is too near the
/// foot of the picture for that.
fn past_reduction((reductions, threshold): &(Vec<f32>, f32)) -> Flat {
  let (mut flat, plot) = (Flat::default(), past_plot());
  let last = (reductions.len() - 1) as f32;
  let edge: Vec<[f32; 2]> = (reductions.iter().enumerate())
    .map(|(block, reduction)| plot.at([block as f32 / last, LEVELS.turn(-reduction)]))
    .collect();
  flat.curve(&edge, 1.8, INK);
  flat.line(
    plot.at([0.0, *threshold]),
    plot.at([1.0, *threshold]),
    1.2,
    TRACE,
  );
  let [x, y] = plot.at([1.0, *threshold]);
  let top = if y + 12.0 < plot.max.y {
    y + 4.0
  } else {
    y - 10.0
  };
  flat.legend("THRESHOLD", [x - 56.0, top], 6.0, 1.0, TRACE);
  flat
}

/// The panel as potpot draws it.
///
/// It holds its meshes, each made once.
struct Compressor {
  /// The renderer, which also finds the pointer on the scene.
  renderer: Renderer,
  /// What each of the scene's objects is, in the same order.
  targets: Vec<Option<Target>>,
  /// Everything that never moves or changes.
  fixed: Drawn,
  screens: [Screen; 2],
  glasses: [Material; 2],
  /// The lines and lettering on each screen.
  grids: [Vec<Object>; 2],
  /// The curve and its dot.
  ///
  /// They are drawn from the parameters and whether the dot is held.
  curve: Picture<(Vec<f32>, bool)>,
  /// The dot that rides the curve, drawn from where it is.
  riding: Picture<[f32; 2]>,
  past_level: Picture<Vec<f32>>,
  past_reduction: Picture<(Vec<f32>, f32)>,
  /// Where the dot on the bend was last drawn.
  bend: [f32; 2],
  /// A knob's body and the line painted on it.
  knob: [Arc<Mesh>; 2],
  /// How far across each knob's middle is.
  ///
  /// Two are under each screen, and the last is under the meters.
  knob_xs: [f32; KNOBS],
  /// How far across each meter's middle is.
  meter_xs: [f32; METERS.len()],
  /// The meter of a level and the meter of the reduction.
  ///
  /// The reduction's lights are all of one color.
  meters: [Meter; 2],
  segment: Arc<Mesh>,
}

impl Compressor {
  fn new(gpu: &Gpu) -> Self {
    let size = Vec2::from(SIZE);
    let ink = Material::ink(Vec3::splat(0.88));
    let mut fixed = Drawn::default();
    fixed.put(
      parts::plate(size.x, size.y, 6.0),
      Vec2::ZERO,
      Material::brushed(Vec3::new(0.24, 0.25, 0.28)),
    );
    fixed.letters("COMPRESSOR", Vec2::new(size.x / 2.0, 22.0), 9.0, ink);
    fixed.screws(size, 16.0, 6.0);

    let screens = [CURVE_GLASS, PAST_GLASS].map(|glass| Screen::new(glass.x, glass.y));
    let glasses = screens.map(|_| shared::glass());
    for ((screen, glass), middle) in screens.iter().zip(glasses).zip([CURVE, PAST_SCREEN]) {
      fixed.put(screen.face(), middle, glass);
      fixed.put(screen.bezel(), middle, Material::glossy(BLACK));
    }
    // what the wide screen's two lines are, each named under it in paint of its own color
    let paint = |[red, green, blue, _]: Color| Material::ink(Vec3::new(red, green, blue));
    // each at an end of the screen, clear of the knobs under its middle
    let named = [("LEVEL IN", -130.0, LOUD_EDGE), ("REDUCTION", 130.0, INK)];
    for (name, aside, color) in named {
      let at = Vec2::new(PAST_SCREEN.x + aside, CAPTION_Y);
      fixed.letters(name, at, 6.5, paint(color));
    }

    // the gaps the meters' lights sit in, with what each shows over it
    let level = Meter::ladder(18, 12.0, METER_FOOT - METER_TOP);
    let reduced = Meter {
      yellow_from: 0.0,
      red_from: 2.0,
      ..level
    };
    // the meters, and the knob under them, in the room right of the wide screen
    let across = |middle: Vec2, glass: Vec2| [middle.x - glass.x / 2.0, middle.x + glass.x / 2.0];
    let room = [across(PAST_SCREEN, PAST_GLASS)[1], size.x];
    let meter_xs = shared::centered(room, METER_SLOT);
    let seat = Arc::new(level.seat());
    for ((_, name), x) in METERS.into_iter().zip(meter_xs) {
      fixed.extend(level.seats(&seat, x, METER_FOOT));
      fixed.letters(name, Vec2::new(x, METER_TOP - 14.0), 6.5, ink);
    }

    let knob = Knob::chicken_head(26.0, 22.0);
    let scale = Arc::new(parts::ticks(
      11,
      parts::KNOB_SWEEP,
      knob.radius + 6.0,
      knob.radius + 11.0,
      5,
    ));
    let [threshold, ratio] = shared::centered(across(CURVE, CURVE_GLASS), KNOB_SLOT);
    let [attack, release] = shared::centered(across(PAST_SCREEN, PAST_GLASS), KNOB_SLOT);
    let knob_xs = [threshold, ratio, attack, release, layout::middle(room)];
    for (control, (name, x)) in KNOB_NAMES.into_iter().zip(knob_xs).enumerate() {
      let on_panel = Mat4::from_translation(Vec3::new(x, KNOB_Y, 0.0));
      fixed.place(&scale, on_panel, ink, None);
      fixed.letters(name, Vec2::new(x, NAME_Y), 7.0, ink);
      fixed.readout(Vec2::new(x, READOUT_Y), READOUT, control);
    }

    let [curve_glass, past_glass] = glasses;
    Self {
      renderer: Renderer::new(gpu.device, gpu.format),
      targets: Vec::new(),
      fixed,
      screens,
      glasses,
      grids: [
        parts::picture(&curve_grid(), CURVE.x, CURVE.y, curve_glass, 0),
        parts::picture(
          &past_grid(),
          PAST_SCREEN.x,
          PAST_SCREEN.y,
          past_glass,
          LOUD_OBJECTS,
        ),
      ],
      curve: Picture::new(),
      riding: Picture::new(),
      past_level: Picture::new(),
      past_reduction: Picture::new(),
      bend: [0.0; 2],
      knob: [knob.body(), knob.line()].map(Arc::new),
      knob_xs,
      meter_xs,
      segment: Arc::new(level.segment()),
      meters: [level, reduced],
    }
  }

  /// Returns the scene and what each of its objects is.
  ///
  /// `values` is where each parameter stands, `feed` is what the audio thread wrote, and `held`
  /// is the control that is held down.
  fn scene(
    &mut self,
    values: &[f32],
    feed: &[f32],
    held: Option<usize>,
  ) -> (Scene, Vec<Option<Target>>) {
    // the scene owns this frame's objects, so the ones that never move are copied into it
    let mut drawn = self.fixed.clone();
    let [curve_screen, past_screen] = self.screens;
    let [red, green, blue, _] = TRACE;
    let mut lamps: Vec<_> =
      (curve_screen.lamps(CURVE.x, CURVE.y, Vec3::new(red, green, blue), 0.8))
        .chain(past_screen.lamps(PAST_SCREEN.x, PAST_SCREEN.y, Vec3::new(1.0, 0.7, 0.3), 0.8))
        .collect();

    let plastic = [Material::satin(CREAM), Material::matte(BLACK)];
    for (control, x) in self.knob_xs.into_iter().enumerate() {
      let at = Vec2::new(x, KNOB_Y);
      drawn.knob(&self.knob, plastic, at, values[control], control);
    }

    let [level, reduced] = self.meters;
    for ((slot, _), x) in METERS.into_iter().zip(self.meter_xs) {
      if slot == REDUCTION {
        // the reduction grows down from the top: its meter is a level's turned half around
        let shown = REDUCTIONS.turn(feed[slot]);
        let turned =
          Mat4::from_translation(Vec3::new(x, METER_TOP, 0.0)) * Mat4::from_rotation_z(PI);
        drawn.extend(
          (reduced.lights(&self.segment, 0.0, 0.0, shown, None)).map(|light| Object {
            transform: turned * light.transform,
            ..light
          }),
        );
      } else {
        let shown = METERED.turn(feed[slot]);
        drawn.extend(level.lights(&self.segment, x, METER_FOOT, shown, None));
        lamps.extend(level.lamp(x, METER_FOOT, shown));
      }
    }

    // each picture is made again only when what it shows is new
    let [curve_glass, past_glass] = self.glasses;
    let on_bend = held == Some(BEND);
    let draw = |(values, held): &(Vec<f32>, bool)| curve(values, *held);
    let under = self.grids[0].len();
    (self.curve).show((values.to_vec(), on_bend), draw, CURVE, curve_glass, under);
    let under = under + self.curve.objects.len();
    let at = on_curve(feed[IN], values);
    (self.riding).show(at, |at| riding(*at), CURVE, curve_glass, under);

    let (levels, reductions) = feed[PAST..].split_at(BLOCKS);
    (self.past_level).show(
      levels.to_vec(),
      |levels| past_level(levels),
      PAST_SCREEN,
      past_glass,
      0,
    );
    let under = LOUD_OBJECTS + self.grids[1].len();
    let from = (reductions.to_vec(), values[THRESHOLD]);
    (self.past_reduction).show(from, past_reduction, PAST_SCREEN, past_glass, under);

    let [curve_grid, past_grid] = &self.grids;
    let pictures = [
      curve_grid,
      &self.curve.objects,
      &self.riding.objects,
      &self.past_level.objects,
      past_grid,
      &self.past_reduction.objects,
    ];
    for picture in pictures {
      drawn.extend(picture.iter().cloned());
    }

    let scene = Scene {
      size: SIZE.into(),
      heights: [-8.0, 40.0],
      lean: Vec2::new(0.0, -0.25),
      // dimmed, so that what the screens show stands out
      light: Light {
        color: Vec3::splat(0.7),
        ..Light::default()
      },
      look: Look::default(),
      objects: drawn.objects,
      lamps,
    };
    (scene, drawn.targets)
  }
}

impl Skin for Compressor {
  fn prepare(&mut self, gpu: &Gpu, view: &View, over: &mut Flat) {
    self.bend = bend(view.values);
    let (scene, targets) = self.scene(view.values, view.feed, view.held);
    (self.renderer).prepare(gpu.device, gpu.queue, &scene, view.pixels);
    self.targets = targets;
    for (control, x) in self.knob_xs.into_iter().enumerate() {
      shared::written(over, view, control, Vec2::new(x, READOUT_Y), LETTERS);
    }
  }

  fn draw(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView, place: [u32; 4]) {
    shared::render(&self.renderer, encoder, target, place);
  }

  fn changed(&self) -> Option<&[[u32; 4]]> {
    self.renderer.changed()
  }

  fn settled(&self) -> bool {
    self.renderer.settled()
  }

  fn target_at(&self, at: [f32; 2]) -> Option<Target> {
    // the dot lies flat on the screen, so it is found by its place and the rest by the scene
    if shared::dot_at(&[self.bend], at, DOT_REACH).is_some() {
      return Some(Target::Control(BEND));
    }
    self.targets[self.renderer.pick(at.into())?]
  }
}

/// A made-up drum loop, a sample at a time.
///
/// It is a kick on every beat, softer in every other bar, a burst of noise on every other beat,
/// a tick between the beats and a low note under it all.
struct Drums {
  noise: Noise,
  /// The count of samples since the two bars it repeats began.
  sample: usize,
  /// How far the low note is through its turn, in radians.
  low: f32,
}

impl Drums {
  /// The count of samples in a beat, of which a bar has four.
  const BEAT: usize = (RATE / 2.0) as usize;

  /// Returns the next sample.
  fn sample(&mut self) -> f32 {
    let beats = self.sample / Self::BEAT;
    let since = (self.sample % Self::BEAT) as f32 / RATE;
    let soft = if beats < 4 { 1.0 } else { 0.45 };
    let kick = 0.8 * soft * (-since * 9.0).exp() * (TAU * 55.0 * since).sin();
    let hiss = 2.0 * self.noise.sample();
    let snare = if beats % 2 == 1 {
      0.45 * (-since * 16.0).exp() * hiss
    } else {
      0.0
    };
    let half = (self.sample % (Self::BEAT / 2)) as f32 / RATE;
    let tick = 0.1 * (-half * 60.0).exp() * hiss;
    let low = 0.12 * self.low.sin();
    // both are kept small, so that neither loses its last digits as the loop goes on
    self.sample = (self.sample + 1) % (Self::BEAT * 8);
    self.low = (self.low + TAU * 82.0 / RATE) % TAU;
    kick + snare + tick + low
  }
}

/// Returns the level in decibels of a sample of a given size.
fn decibels(size: f32) -> f32 {
  20.0 * size.abs().max(1e-3).log10()
}

/// Returns the function that compresses one block of the sound.
///
/// The sound is made up. The parameters are as `values` has them, and what is seen of the sound
/// goes into `feed`.
///
/// It works a sample at a time within the block, the way a plugin's audio thread does, and writes
/// the feed at the end of the block.
fn sound(feed: Feed, values: Arc<[AtomicU32]>) -> impl FnMut() + Send {
  let samples = (RATE * BLOCK.as_secs_f32()) as usize;
  let mut drums = Drums {
    noise: Noise::new(),
    sample: 0,
    low: 0.0,
  };
  // the decibels taken off at the last sample, and the meters' two levels
  let mut reduced = 0.0_f32;
  let mut shown = [LEVELS.plain(0.0); 2];
  let mut past = [[LEVELS.plain(0.0); BLOCKS], [0.0; BLOCKS]];
  move || {
    let plain = |parameter: usize, axis: Axis| axis.plain(shared::value(&values, parameter));
    let (threshold, ratio) = (plain(THRESHOLD, LEVELS), plain(RATIO, RATIOS));
    let makeup = plain(MAKEUP, MAKEUPS);
    // the part of its way to a new reduction that is left after one sample
    let left = |milliseconds: f32| (-1000.0 / (milliseconds * RATE)).exp();
    let (attack, release) = (left(plain(ATTACK, ATTACKS)), left(plain(RELEASE, RELEASES)));

    let (mut largest, mut most) = ([0.0_f32; 2], 0.0_f32);
    for _ in 0..samples {
      let dry = drums.sample();
      // the reduction the level asks for, which the one in use goes toward at the pace of the
      // attack when it is to grow and of the release when it is to shrink
      let wanted = reduction(decibels(dry), threshold, ratio);
      let left = if wanted > reduced { attack } else { release };
      reduced = wanted + (reduced - wanted) * left;
      let wet = dry * 10.0_f32.powf((makeup - reduced) / 20.0);
      largest = [largest[IN].max(dry.abs()), largest[OUT].max(wet.abs())];
      most = most.max(reduced);
    }

    // a meter jumps up to a level and falls back slowly
    for (shown, largest) in shown.iter_mut().zip(largest) {
      *shown = decibels(largest).max(*shown - 1.5);
    }
    feed.set(IN, shown[IN]);
    feed.set(OUT, shown[OUT]);
    feed.set(REDUCTION, most);
    let [levels, reductions] = &mut past;
    levels.rotate_left(1);
    reductions.rotate_left(1);
    levels[BLOCKS - 1] = decibels(largest[IN]);
    reductions[BLOCKS - 1] = most;
    for (slot, number) in (PAST..).zip(levels.iter().chain(reductions.iter())) {
      feed.set(slot, *number);
    }
  }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut settings = Settings::new("potpot compressor", SIZE);
  let feed = Feed::new(PAST + BLOCKS * 2);
  settings.feed = feed.clone();
  let (dials, controls) = wiring();
  let (params, values) = shared::params(dials);
  let sound = shared::window::Sound {
    block: Box::new(sound(feed, values)),
    every: BLOCK,
    pictured_after: PICTURED_AFTER,
  };
  shared::window::run(settings, params, controls, Some(sound), Compressor::new)
}
