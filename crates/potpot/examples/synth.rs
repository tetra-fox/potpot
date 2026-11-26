//! The front of a small synthesizer, opened by `just example synth`.
//!
//! `just render synth out.png` draws it to a file in place of the window, with the made-up run
//! of notes playing.
//!
//! Three sections, each with a screen that draws what its parameters make:
//!
//! - `OSCILLATOR`: a switch of four places picks the wave and a knob tunes it. The scope draws
//!   the wave's harmonics added up, each at the gain the filter has at its frequency. The filter
//!   also shifts each harmonic in time, which the scope leaves out.
//! - `FILTER`: the puck of the pad is dragged across for the cutoff and up for the resonance,
//!   and a knob beside it turns each of the two. The screen draws the filter's response.
//! - `ENVELOPE`: four faders set the attack, the decay, the sustain and the release. The corners
//!   of the envelope on the screen are dots on the same parameters: the first is dragged across
//!   for the attack, the second across for the decay and up for the sustain, and the last across
//!   for the release. A stage is as wide in the picture as its parameter is far along its
//!   dial, up to a quarter of the picture, so the picture is not drawn to a scale of time.
//!
//! A control names the parameters it turns, and several controls name the same one. So the
//! puck turns the two knobs, a dot slides its faders, and each of those moves the puck or the
//! dot.
//!
//! Along the bottom is a keybed of three octaves. A press on a key plays its note, harder the
//! nearer the front of the key it lands, and a slide with the button down plays each key it
//! comes to. A thread standing in for a plugin's audio thread takes the notes from a [`Notes`]
//! and runs the envelope for them, with no sound. Through a [`Feed`] it hands back what the
//! screens show of a note:
//!
//! - the white dot that rides the envelope while a key is held and through its release
//! - the note's pitch, which the scope draws the wave at, and how hard it was struck, which the
//!   scope draws the wave as large as
//!
//! The `HOST` button starts a made-up run of notes on that thread, as a host would play them.
//! The thread shows them through the same [`Notes`], and their keys go down by themselves.

mod shared;

use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use std::time::Duration;

use potpot::glam::{Mat4, Vec2, Vec3};
use potpot::parts::{self, Button, Fader, Keybed, Knob, Pad, Screen};
use potpot::{Light, Look, Material, Mesh, Renderer, Scene};
use potpot_window::controls::axis::Axis;
use potpot_window::controls::flat::{Color, Flat};
use potpot_window::controls::interaction::{Control, Dial, Drag, Push, Readout, Target, Touch};
use potpot_window::controls::response::{Band, Kind};
use potpot_window::{Feed, Gpu, Heard, Notes, Played, Pressed, Settings, Skin, View, wgpu};

use crate::shared::{CYAN, Drawn, Picture, Plot, SEAT};

const SIZE: [f32; 2] = [920.0, 650.0];
/// How far the view leans.
const LEAN: Vec2 = Vec2::new(0.0, -0.25);

// the parameters. each has a knob, a fader or a button at the same place among the controls,
// and after those come the pad's puck and the three dots on the envelope
const WAVE: usize = 0;
const TUNE: usize = 1;
const CUTOFF: usize = 2;
const RESONANCE: usize = 3;
const ATTACK: usize = 4;
const DECAY: usize = 5;
const SUSTAIN: usize = 6;
const RELEASE: usize = 7;
/// Whether the made-up run of notes plays.
const HOST: usize = 8;
const PARAMETERS: usize = 9;
/// The envelope's four parameters, in the order drawn from.
const STAGES: [usize; 4] = [ATTACK, DECAY, SUSTAIN, RELEASE];
const PUCK: usize = PARAMETERS;
const ATTACK_DOT: usize = PARAMETERS + 1;
const DECAY_DOT: usize = PARAMETERS + 2;
const RELEASE_DOT: usize = PARAMETERS + 3;

/// The waves the switch picks from, as its readout writes them.
const WAVES: [&str; 4] = ["SINE", "TRI", "SAW", "SQUARE"];
/// The places of the waves among [`WAVES`].
///
/// The wave's parameter lies along them.
const WAVE_PLACES: Axis = Axis::linear([0.0, (WAVES.len() - 1) as f32]);
/// The semitones the note is tuned by.
const TUNES: Axis = Axis::linear([-12.0, 12.0]);
// what the filter's parameters lie along and its response is drawn along
const HERTZ: Axis = Axis::log([20.0, 20000.0]);
const RESONANCES: Axis = Axis::log([0.5, 12.0]);
const DECIBELS: Axis = Axis::linear([-30.0, 24.0]);
// the envelope's times in milliseconds, and its sustain in hundredths of its peak
const ATTACKS: Axis = Axis::log([1.0, 2000.0]);
const DECAYS: Axis = Axis::log([5.0, 2000.0]);
const SUSTAINS: Axis = Axis::linear([0.0, 100.0]);
const RELEASES: Axis = Axis::log([5.0, 4000.0]);
/// The sample rate the response is drawn for, in hertz.
const RATE: f32 = 48000.0;
/// The note the scope draws until one is played.
///
/// It is the A of 110 hertz.
const REST: u8 = 45;
/// The stretch of time the scope draws, in seconds.
///
/// It is two turns of [`REST`].
const SCOPE_TIME: f32 = 2.0 / 110.0;
/// The count of the wave's harmonics that the scope adds up.
const HARMONICS: u16 = 64;
/// How far from the scope's middle a level of one is drawn.
///
/// It is a part of the height of the scope's picture.
const SCOPE_REACH: f32 = 0.36;

// the slots of the feed: the envelope's level from zero to one, how far along the envelope the
// note is, the note last played and how hard it was struck as a part of the hardest
const LEVEL: usize = 0;
/// How far along its envelope the note is.
///
/// It is the count of stages gone through and the part of the one the note is in. The stages
/// are the attack, the decay, the hold and the release. Under zero no note sounds.
const ALONG: usize = 1;
const PITCH: usize = 2;
const STRUCK: usize = 3;
const SLOTS: usize = 4;
/// The time between two blocks of the thread that plays.
const BLOCK: Duration = Duration::from_millis(10);
/// The time the rider takes across the hold, in seconds.
///
/// The hold lasts as long as the key is down, and the rider waits at its end.
const HOLD: f32 = 1.0;
/// The made-up run of notes, each [`STEP`] blocks long.
const RUN: [u8; 8] = [48, 55, 60, 63, 67, 63, 60, 55];
const STEP: u32 = 24;
/// The blocks of a step that its note is down for.
const DOWN_FOR: u32 = 20;
/// The count of blocks that a picture is drawn after.
///
/// The third note of the run is down by then, and in its decay.
const PICTURED_AFTER: u32 = STEP * 2 + 12;

// the three screens: how far down their middles are, how long their faces are, and each one's
// middle across the panel with the width of its face
const SCREEN_Y: f32 = 112.0;
const SCREEN_LENGTH: f32 = 124.0;
const SCOPE: (f32, f32) = (142.0, 204.0);
const RESPONSE: (f32, f32) = (420.0, 280.0);
const ENVELOPE: (f32, f32) = (740.0, 280.0);
/// The distance from the edge of a face in to its picture.
const INSET: f32 = 10.0;

// how far across the lines cut in the panel between two sections are, their middle down the
// panel, and how long they are
const DIVIDERS: [f32; 2] = [264.0, 580.0];
const DIVIDER_Y: f32 = 235.0;
const DIVIDER: f32 = 400.0;
/// How far down the line cut over the keys is.
const KEYS_LINE: f32 = 462.0;
// the middle of the keybed, and the button beside it that starts the made-up run
const KEYS: Vec2 = Vec2::new(520.0, 552.0);
const HOST_BUTTON: Vec2 = Vec2::new(80.0, 540.0);
// the oscillator's two knobs, one under the other, and the sweep of the switch from its first
// place to its last
const SWITCH: Vec2 = Vec2::new(96.0, 256.0);
const SWITCH_SWEEP: f32 = PI * 0.8;
const TUNING: Vec2 = Vec2::new(96.0, 370.0);
/// How far right of an oscillator's knob its readout is.
const BESIDE: f32 = 100.0;
/// The radius of the oscillator's knobs.
const LARGE: f32 = 28.0;
const SMALL: f32 = 22.0;
// the pad, and the filter's two knobs beside it
const PAD: Vec2 = Vec2::new(376.0, 306.0);
const PAD_SIDE: f32 = 136.0;
const FILTER_KNOBS: [(usize, &str, Vec2); 2] = [
  (CUTOFF, "CUTOFF", Vec2::new(516.0, 240.0)),
  (RESONANCE, "RES", Vec2::new(516.0, 352.0)),
];
// the envelope's faders: their names, the room each has across the panel, their middle down the
// panel, and how far they slide
const FADERS: [&str; 4] = ["ATTACK", "DECAY", "SUSTAIN", "RELEASE"];
const FADER_SLOT: f32 = 62.0;
const FADER_Y: f32 = 302.0;
const TRAVEL: f32 = 110.0;
// how far under a knob's edge its name and its readout are, and how far down the faders'
const NAME_UNDER: f32 = 22.0;
const READOUT_UNDER: f32 = 46.0;
const FADER_NAME_Y: f32 = 392.0;
const FADER_READOUT_Y: f32 = 418.0;
/// Half a readout's width and height.
const READOUT: Vec2 = Vec2::new(27.0, 9.0);
const LETTERS: f32 = 6.5;

/// How far from its middle a dot of the envelope reaches.
const DOT_REACH: f32 = 6.0;
const GREEN: Color = [0.4, 1.0, 0.5, 1.0];
const AMBER: Color = [1.0, 0.7, 0.2, 1.0];

/// Returns a screen whose face is `width` wide.
fn screen(width: f32) -> Screen {
  Screen::new(width, SCREEN_LENGTH)
}

/// Returns the place of the picture of one of the screens.
fn plot((x, width): (f32, f32)) -> Plot {
  Plot::of(&screen(width), Vec2::new(x, SCREEN_Y), INSET)
}

/// Returns the width of a stage of the envelope at its longest.
///
/// The attack, the decay and the release each take up to a quarter of the picture, and the time
/// the note is held takes a quarter.
fn stage() -> f32 {
  plot(ENVELOPE).size().x / 4.0
}

/// Returns the wave a value of its parameter picks.
///
/// The wave is given by its place among [`WAVES`].
fn wave(value: f32) -> usize {
  WAVE_PLACES.plain(value).round() as usize
}

/// Returns every parameter and every control.
fn wiring() -> (Vec<Dial>, Vec<Control>) {
  // the wave is one of four, which its readout writes and reads by name
  let mut wave = Dial::new(
    WAVE_PLACES.turn(2.0),
    |value| WAVE_PLACES.plain(value),
    |plain| WAVE_PLACES.turn(plain),
    |value| WAVES[wave(value)].to_owned(),
    |text| {
      let named = |name: &&str| name.eq_ignore_ascii_case(text.trim());
      (WAVES.iter().position(named)).map(|place| WAVE_PLACES.turn(place as f32))
    },
  );
  wave.step = Some(1.0);
  // in the order of the parameters
  let dials = vec![
    wave,
    Dial::along(TUNES, 0.0, Some(1.0), 0, " ST"),
    Dial::along(HERTZ, 1200.0, None, 0, " HZ"),
    Dial::along(RESONANCES, 2.0, None, 2, ""),
    Dial::along(ATTACKS, 40.0, None, 0, " MS"),
    Dial::along(DECAYS, 300.0, None, 0, " MS"),
    Dial::along(SUSTAINS, 60.0, None, 0, "%"),
    Dial::along(RELEASES, 700.0, None, 0, " MS"),
    Dial::linear([0.0, 1.0], 0.0, Some(1.0), 0, ""),
  ];

  let readout = Readout::new(7);
  let along = |parameter: usize, way: [f32; 2], length: f32| {
    let mut control = Control::new(parameter, readout);
    control.feel.drag = Drag::Along {
      way,
      turn: 1.0 / length,
    };
    control
  };
  let mut controls: Vec<Control> = (0..PARAMETERS)
    .map(|parameter| match parameter {
      // three steps in a third of the drag that turns a knob all the way
      WAVE => along(WAVE, [0.0, -1.0], 70.0),
      // a fader's cap goes as far up the panel as the pointer does
      ATTACK..=RELEASE => along(parameter, [0.0, -1.0], TRAVEL),
      HOST => {
        let mut button = Control::new(HOST, readout);
        button.touch = Touch::Press(Push::Latching);
        button
      }
      _ => Control::new(parameter, readout),
    })
    .collect();
  // dragged over the room the puck's middle has
  let room = Pad::new(PAD_SIDE, PAD_SIDE).travel().into();
  controls.push(Control::pad(CUTOFF, RESONANCE, room, readout));
  // each dot of the envelope goes across one stage of the picture for the whole of its turn,
  // and the one between the decay and the sustain up the whole picture as well
  controls.push(along(ATTACK, [1.0, 0.0], stage()));
  let room = [stage(), plot(ENVELOPE).size().y];
  controls.push(Control::pad(DECAY, SUSTAIN, room, readout));
  controls.push(along(RELEASE, [1.0, 0.0], stage()));
  (dials, controls)
}

/// Returns the filter for where its two parameters stand.
///
/// Each stands at a part of its whole turn.
fn filter(cutoff: f32, resonance: f32) -> Band {
  Band {
    kind: Kind::HighCut,
    frequency: HERTZ.plain(cutoff),
    gain: 0.0,
    q: RESONANCES.plain(resonance),
  }
}

/// Returns the frequency in hertz of a tuned note.
///
/// The note is tuned by `semitones`.
fn hertz(note: f32, semitones: f32) -> f32 {
  440.0 * 2.0_f32.powf((note - 69.0 + semitones) / 12.0)
}

/// Returns the wave's harmonics, sized by the filter and added.
///
/// The gain is the one the filter has at the harmonic's frequency. The sum is drawn over
/// [`SCOPE_TIME`]. `picked`, `tune`, `cutoff` and `resonance` are where the wave, the tuning,
/// the cutoff and the resonance stand. `pitch` is the note that sounds, and `struck` how hard it
/// was struck, as the feed has them.
fn scope(&[picked, tune, cutoff, resonance, pitch, struck]: &[f32; 6]) -> Flat {
  let (mut flat, plot) = (Flat::default(), plot(SCOPE));
  plot.divide(&mut flat, 8, 4);
  let filter = filter(cutoff, resonance);
  let note = hertz(pitch, TUNES.plain(tune));
  // the softest note is drawn a third as large as the hardest
  let reach = SCOPE_REACH * (1.0 + 2.0 * struck) / 3.0;
  let shape = wave(picked);
  // each harmonic's size, with the filter's gain at it: a sine has the first alone, a triangle
  // and a square the odd ones, and a saw them all
  let sizes: Vec<f32> = (1..=HARMONICS)
    .map(|harmonic| {
      let (number, odd) = (f32::from(harmonic), harmonic % 2 == 1);
      let size = match shape {
        0 if harmonic == 1 => 1.0,
        // every other odd one is turned over
        1 if odd => 8.0 / (PI * PI * number * number) * if harmonic % 4 == 1 { 1.0 } else { -1.0 },
        2 => 2.0 / (PI * number),
        3 if odd => 4.0 / (PI * number),
        _ => 0.0,
      };
      let frequency = note * number;
      if size == 0.0 || frequency >= RATE / 2.0 {
        return 0.0;
      }
      size * 10.0_f32.powf(filter.gain_at(frequency, RATE) / 20.0)
    })
    .collect();
  let points: Vec<[f32; 2]> = (0..=200u16)
    .map(|step| {
      let across = f32::from(step) / 200.0;
      let turn = across * SCOPE_TIME * note * TAU;
      let level: f32 = (sizes.iter().zip(1u16..))
        .map(|(size, harmonic)| size * (turn * f32::from(harmonic)).sin())
        .sum();
      plot.at([across, (0.5 + reach * level).clamp(0.0, 1.0)])
    })
    .collect();
  flat.curve(&points, 1.8, GREEN);
  flat
}

/// Returns the filter's response over frequency.
///
/// `cutoff` and `resonance` are where the two stand. The curve ends where it goes under
/// the foot of the picture.
fn response(&[cutoff, resonance]: &[f32; 2]) -> Flat {
  let (mut flat, plot) = (Flat::default(), plot(RESPONSE));
  for frequency in [100.0, 1000.0, 10000.0] {
    let across = HERTZ.turn(frequency);
    flat.line(
      plot.at([across, 0.0]),
      plot.at([across, 1.0]),
      1.0,
      shared::GRID,
    );
  }
  for gain in [-24.0, -12.0, 0.0, 12.0] {
    let up = DECIBELS.turn(gain);
    flat.line(plot.at([0.0, up]), plot.at([1.0, up]), 1.0, shared::GRID);
  }
  let filter = filter(cutoff, resonance);
  let foot = DECIBELS.plain(0.0);
  let mut points = Vec::new();
  // the gain at one frequency before, for the place between the two where the curve meets
  // the foot
  let mut before: Option<(f32, f32)> = None;
  for step in 0..=200u16 {
    let across = f32::from(step) / 200.0;
    let gain = filter.gain_at(HERTZ.plain(across), RATE);
    if gain < foot {
      if let Some((last, over)) = before {
        let part = (over - foot) / (over - gain);
        points.push(plot.at([last + (across - last) * part, 0.0]));
      }
      break;
    }
    points.push(plot.at([across, DECIBELS.turn(gain)]));
    before = Some((across, gain));
  }
  flat.area(&points, plot.max.y, [0.03, 0.14, 0.17, 1.0]);
  flat.curve(&points, 2.0, CYAN);
  flat
}

/// Returns the envelope's corners, as places in its picture.
///
/// They are its start, the end of the attack, the end of the decay, the end of the time the note
/// is held and the end of the release.
///
/// `attack`, `decay`, `sustain` and `release` are where the four stand.
fn corners([attack, decay, sustain, release]: [f32; 4]) -> [[f32; 2]; 5] {
  let ends = [
    0.0,
    attack,
    attack + decay,
    attack + decay + 1.0,
    attack + decay + 1.0 + release,
  ];
  let levels = [0.0, 1.0, sustain, sustain, 0.0];
  std::array::from_fn(|corner| [ends[corner] / 4.0, levels[corner]])
}

/// Returns how far a stage has gone toward its end.
///
/// `part` is the part of its time that has passed. Each stage closes most of what is left of
/// its way early on.
fn eased(part: f32) -> f32 {
  1.0 - (1.0 - part).powi(3)
}

/// Returns the white dot that rides the envelope.
///
/// `stages` is as [`corners`] takes it. `along` and `level` are as the feed has them: the dot
/// is as far across its stage as the note is through it, and as high as the envelope's level.
/// With no note it rests on the envelope's start.
fn rider(&(stages, along, level): &([f32; 4], f32, f32)) -> Flat {
  let corners = corners(stages);
  let at = if along < 0.0 {
    corners[0]
  } else {
    let stage = (along as usize).min(3);
    let ([from, _], [to, _]) = (corners[stage], corners[stage + 1]);
    [from + (to - from) * (along - stage as f32).min(1.0), level]
  };
  let (mut flat, at) = (Flat::default(), plot(ENVELOPE).at(at));
  flat.line(at, at, 6.0, [1.0, 1.0, 1.0, 1.0]);
  flat
}

/// Returns where the three dots of the envelope are.
///
/// They are on the ends of the attack, the decay and the release. `stages` is as [`corners`]
/// takes it.
fn dots(stages: [f32; 4]) -> [[f32; 2]; 3] {
  let corners = corners(stages);
  [1, 2, 4].map(|corner| plot(ENVELOPE).at(corners[corner]))
}

/// Returns the envelope, with a dot on each dragged corner.
///
/// `stages` is as [`corners`] takes it, and `held` is the dot that is held down, counted
/// along the envelope.
fn envelope(&(stages, held): &([f32; 4], Option<usize>)) -> Flat {
  let (mut flat, plot) = (Flat::default(), plot(ENVELOPE));
  plot.divide(&mut flat, 4, 4);
  let corners = corners(stages);
  let mut curve = Vec::new();
  for stage in corners.windows(2) {
    let ([from, low], [to, high]) = (stage[0], stage[1]);
    for step in 0..24u8 {
      let part = f32::from(step) / 24.0;
      curve.push(plot.at([from + (to - from) * part, low + (high - low) * eased(part)]));
    }
  }
  curve.push(plot.at(corners[4]));
  flat.area(&curve, plot.max.y, [0.22, 0.12, 0.02, 1.0]);
  flat.curve(&curve, 2.0, AMBER);
  for (dot, at) in dots(stages).into_iter().enumerate() {
    shared::dot(
      &mut flat,
      at,
      DOT_REACH,
      [1.0, 0.9, 0.7, 1.0],
      held == Some(dot),
    );
  }
  flat
}

/// Returns the dot of the envelope that a control moves.
///
/// The dots are counted along the envelope.
///
/// A dot is held down while it is dragged, and so it is while a fader on its parameters is.
fn dot_of(control: usize) -> Option<usize> {
  match control {
    ATTACK | ATTACK_DOT => Some(0),
    DECAY | SUSTAIN | DECAY_DOT => Some(1),
    RELEASE | RELEASE_DOT => Some(2),
    _ => None,
  }
}

/// The panel as potpot draws it.
///
/// It holds its meshes, each made once.
struct Synth {
  /// The renderer, which also finds the pointer on the scene.
  renderer: Renderer,
  /// What each of the scene's objects is, in the same order.
  targets: Vec<Option<Target>>,
  /// Everything that never moves or changes.
  fixed: Drawn,
  /// The material of each screen's face.
  ///
  /// They are the scope's, the response's and the envelope's.
  glasses: [Material; 3],
  scope: Picture<[f32; 6]>,
  response: Picture<[f32; 2]>,
  envelope: Picture<([f32; 4], Option<usize>)>,
  rider: Picture<([f32; 4], f32, f32)>,
  /// Where each dot of the envelope was last drawn.
  dots: [[f32; 2]; 3],
  /// The switch's body and the line painted on it.
  switch: [Arc<Mesh>; 2],
  /// The body and the painted line of each size of knob.
  ///
  /// The first is the knob that tunes and the second a knob of the filter.
  knobs: [[Arc<Mesh>; 2]; 2],
  /// A fader's cap and its stripe.
  cap: [Arc<Mesh>; 2],
  /// How far across each fader's middle is.
  ///
  /// The faders are under the envelope's screen.
  faders: [f32; FADERS.len()],
  pad: Pad,
  puck: Arc<Mesh>,
  keybed: Keybed,
  /// A white key and a black one.
  keys: [Arc<Mesh>; 2],
  /// The strip in a black key's top, lit while the key is down.
  strip: Arc<Mesh>,
  host: Button,
  host_body: Arc<Mesh>,
  /// Each readout's control and its middle.
  readouts: Vec<(usize, Vec2)>,
}

impl Synth {
  fn new(gpu: &Gpu) -> Self {
    let size = Vec2::from(SIZE);
    let ink = Material::matte(Vec3::splat(0.9));
    let hole = Material::matte(SEAT);
    let mut fixed = Drawn::default();
    let plastic = Material {
      grain: 0.22,
      ..Material::satin(Vec3::new(0.08, 0.2, 0.36))
    };
    fixed.put(parts::plate(size.x, size.y, 6.0), Vec2::ZERO, plastic);
    fixed.screws(size, 16.0, 6.0);

    // each section's name over its screen, and a line cut in the panel between two sections
    let sections = [
      ("OSCILLATOR", SCOPE),
      ("FILTER", RESPONSE),
      ("ENVELOPE", ENVELOPE),
    ];
    let glasses = sections.map(|(_, (_, width))| shared::glass(&screen(width)));
    for ((name, (x, width)), glass) in sections.into_iter().zip(glasses) {
      let (screen, middle) = (screen(width), Vec2::new(x, SCREEN_Y));
      fixed.letters(name, Vec2::new(x, 26.0), 8.0, ink);
      fixed.put(screen.face(), middle, glass);
      fixed.put(screen.bezel(), middle, Material::glossy(shared::BLACK));
    }
    let cut = |fixed: &mut Drawn, half: Vec2, middle: Vec2| {
      let line = Mesh::cuboid(-half.extend(0.0), half.extend(0.2));
      fixed.put(line, middle, hole);
    };
    for x in DIVIDERS {
      let half = Vec2::new(0.7, DIVIDER / 2.0);
      cut(&mut fixed, half, Vec2::new(x, DIVIDER_Y));
    }
    let half = Vec2::new(size.x / 2.0 - 30.0, 0.7);
    cut(&mut fixed, half, Vec2::new(size.x / 2.0, KEYS_LINE));

    // the switch, with a mark and a name at each of its places, and the knob that tunes. each
    // has its name over its readout beside it
    let mut readouts = Vec::new();
    let mut named = |fixed: &mut Drawn, control: usize, name: &str, name_at: Vec2| {
      fixed.letters(name, name_at, 6.5, ink);
      let window = name_at + Vec2::Y * (READOUT_UNDER - NAME_UNDER);
      fixed.readout(window, READOUT, control);
      readouts.push((control, window));
    };
    let name_at = Vec2::new(BESIDE, -(READOUT_UNDER - NAME_UNDER) / 2.0);
    let switch = Knob::chicken_head(LARGE, 24.0);
    let scale = |knob: f32, count: u32, sweep: f32, long_every: u32| {
      parts::ticks(count, sweep, knob + 5.0, knob + 9.0, long_every)
    };
    fixed.put(scale(LARGE, 4, SWITCH_SWEEP, 0), SWITCH, ink);
    for (place, name) in ["SIN", "TRI", "SAW", "SQR"].into_iter().enumerate() {
      let angle = (place as f32 / 3.0 - 0.5) * SWITCH_SWEEP;
      let out = Vec2::new(angle.sin(), -angle.cos()) * (LARGE + 20.0);
      fixed.letters(name, SWITCH + out, 5.5, ink);
    }
    named(&mut fixed, WAVE, "WAVE", SWITCH + name_at);
    fixed.put(scale(LARGE, 11, parts::KNOB_SWEEP, 5), TUNING, ink);
    named(&mut fixed, TUNE, "TUNE", TUNING + name_at);
    let on_panel = |at: Vec2| Mat4::from_translation(at.extend(0.0));

    // the pad's surface in its rim, with what a drag each way turns beside it, and the
    // filter's two knobs
    let pad = Pad::new(PAD_SIDE, PAD_SIDE);
    fixed.put(pad.surface(), PAD, Material::glossy(Vec3::splat(0.02)));
    fixed.put(pad.rim(), PAD, Material::chrome(Vec3::splat(0.8)));
    let out = PAD_SIDE / 2.0 + 18.0;
    fixed.letters("CUTOFF", PAD + Vec2::Y * out, 6.5, ink);
    let on_end = on_panel(PAD - Vec2::X * out) * Mat4::from_rotation_z(-FRAC_PI_2);
    fixed.place(&Arc::new(shared::lettering("RES", 6.5)), on_end, ink, None);
    for (control, name, at) in FILTER_KNOBS {
      fixed.put(scale(SMALL, 11, parts::KNOB_SWEEP, 5), at, ink);
      named(
        &mut fixed,
        control,
        name,
        at + Vec2::Y * (SMALL + NAME_UNDER),
      );
    }

    // each fader's slot and scale, with its name and its readout under it
    let (x, width) = ENVELOPE;
    let faders = shared::centered([x - width / 2.0, x + width / 2.0], FADER_SLOT);
    for (control, (name, x)) in (ATTACK..).zip(FADERS.into_iter().zip(faders)) {
      let at = Vec2::new(x, FADER_Y);
      fixed.put(parts::fader_slot(TRAVEL, 5.0), at, hole);
      fixed.put(parts::fader_ticks(11, TRAVEL, 15.0, 20.0, 5), at, ink);
      fixed.letters(name, Vec2::new(x, FADER_NAME_Y), 6.0, ink);
      let window = Vec2::new(x, FADER_READOUT_Y);
      fixed.readout(window, READOUT, control);
      readouts.push((control, window));
    }

    // the keys' seat, and the button beside them with its name under it
    let keybed = Keybed::new(36..=72, 33.0, 128.0);
    fixed.put(keybed.seat(3.0), KEYS, hole);
    let host = Button::round(30.0, 8.0);
    fixed.put(host.seat(2.0), HOST_BUTTON, hole);
    fixed.letters("HOST", HOST_BUTTON + Vec2::Y * 34.0, 6.5, ink);

    // across the middle of the key's top, toward its front
    let half = Vec2::new(keybed.black_width * 0.28, keybed.black_length * 0.04);
    let middle = Vec3::new(0.0, keybed.black_length * 0.3, keybed.black_height);
    let strip = Mesh::cuboid(middle - half.extend(0.0), middle + half.extend(0.4));

    let cap = Fader::console(22.0, 32.0, 13.0);
    Self {
      renderer: Renderer::new(gpu.device, gpu.format),
      targets: Vec::new(),
      fixed,
      glasses,
      scope: Picture::new(),
      response: Picture::new(),
      envelope: Picture::new(),
      rider: Picture::new(),
      dots: [[0.0; 2]; 3],
      switch: [switch.body(), switch.line()].map(Arc::new),
      knobs: [LARGE, SMALL].map(|radius| {
        let knob = Knob::round(radius, radius);
        [knob.body(), knob.line()].map(Arc::new)
      }),
      cap: [cap.body(), cap.line()].map(Arc::new),
      faders,
      puck: Arc::new(pad.puck()),
      pad,
      keys: [keybed.white(), keybed.black()].map(Arc::new),
      strip: Arc::new(strip),
      keybed,
      host_body: Arc::new(host.body()),
      host,
      readouts,
    }
  }

  /// Returns the scene and what each of its objects is.
  ///
  /// `values` is where each parameter stands, `feed` what the thread that plays the notes says
  /// of them, `held` the control that is held down and `notes` the notes whose keys are down.
  fn scene(
    &mut self,
    values: &[f32],
    feed: &[f32],
    held: Option<usize>,
    notes: Pressed,
  ) -> (Scene, Vec<Option<Target>>) {
    // the scene owns this frame's objects, so the ones that never move are copied into it
    let mut drawn = self.fixed.clone();
    let soft = [
      Material::satin(Vec3::splat(0.16)),
      Material::matte(Vec3::splat(0.92)),
    ];
    // the switch turns through its own sweep, from its first place to its last
    let turned = parts::knob_transform(SWITCH.x, SWITCH.y, values[WAVE], SWITCH_SWEEP);
    for (mesh, material) in self.switch.iter().zip(soft) {
      drawn.place(mesh, turned, material, Some(Target::Control(WAVE)));
    }
    let [large, small] = &self.knobs;
    drawn.knob(large, soft, TUNING, values[TUNE], TUNE);
    for (control, _, at) in FILTER_KNOBS {
      drawn.knob(small, soft, at, values[control], control);
    }
    for (control, x) in (ATTACK..).zip(self.faders) {
      let at = Vec2::new(x, FADER_Y);
      drawn.fader(&self.cap, at, TRAVEL, values[control], control);
    }

    // the puck, lit in the color of the response it moves
    let [red, green, blue, _] = CYAN;
    let cyan = Vec3::new(red, green, blue);
    let on_pad = (self.pad).transform(PAD.x, PAD.y, values[CUTOFF], values[RESONANCE]);
    let rubber = Material::silicone(Vec3::splat(0.62));
    drawn.place(
      &self.puck,
      on_pad,
      rubber.lit(cyan, 0.5, self.pad.puck.glow_radius()),
      Some(Target::Control(PUCK)),
    );
    let puck = on_pad.transform_point3(Vec3::ZERO);
    let mut lamps = vec![self.pad.puck.lamp(puck.x, puck.y, cyan, 0.5)];

    // the button, lit while the run plays, and each key, down while its note is. a key is found
    // by where it is on the bed, so no press is on its object
    let playing = values[HOST] > 0.5;
    let depth = Push::Latching.depth(playing, held == Some(HOST));
    let [red, green, blue, _] = AMBER;
    let (amber, level) = (Vec3::new(red, green, blue), f32::from(u8::from(playing)));
    drawn.place(
      &self.host_body,
      self.host.transform(HOST_BUTTON.x, HOST_BUTTON.y, depth),
      rubber.lit(amber, level, self.host.glow_radius()),
      Some(Target::Control(HOST)),
    );
    lamps.push((self.host).lamp(HOST_BUTTON.x, HOST_BUTTON.y, amber, level));
    let [white, black] = &self.keys;
    let ivory = Material::glossy(Vec3::new(0.95, 0.94, 0.9));
    let ebony = Material::glossy(shared::BLACK);
    for note in self.keybed.notes() {
      let (mesh, material) = if Keybed::is_black(note) {
        (black, ebony)
      } else {
        (white, ivory)
      };
      // a key that is down is lit in the envelope's color. a white one glows all over, and a
      // black one stays black under a strip that lights, since a glow would wash it out
      let pressed = f32::from(u8::from(notes.holds(note)));
      let tipped = self.keybed.transform(KEYS.x, KEYS.y, note, pressed);
      if Keybed::is_black(note) {
        drawn.place(mesh, tipped, material, None);
        let strip = material.lit(amber, pressed, self.keybed.black_width);
        drawn.place(&self.strip, tipped, strip, None);
      } else {
        let glowing = material.lit(amber, pressed * 0.4, self.keybed.length);
        drawn.place(mesh, tipped, glowing, None);
      }
    }

    // each screen's picture is made again only when the parameters it shows have moved
    let [scope_glass, response_glass, envelope_glass] = self.glasses;
    let middle = |(x, _): (f32, f32)| Vec2::new(x, SCREEN_Y);
    let of = |parameters: [usize; 4]| parameters.map(|parameter| values[parameter]);
    let [wave, tune, cutoff, resonance] = of([WAVE, TUNE, CUTOFF, RESONANCE]);
    let source = [wave, tune, cutoff, resonance, feed[PITCH], feed[STRUCK]];
    (self.scope).show(source, scope, middle(SCOPE), scope_glass, 0);
    let source = [values[CUTOFF], values[RESONANCE]];
    (self.response).show(source, response, middle(RESPONSE), response_glass, 0);
    let source = (of(STAGES), held.and_then(dot_of));
    (self.envelope).show(source, envelope, middle(ENVELOPE), envelope_glass, 0);
    let (source, under) = (
      (of(STAGES), feed[ALONG], feed[LEVEL]),
      self.envelope.objects.len(),
    );
    (self.rider).show(source, rider, middle(ENVELOPE), envelope_glass, under);
    let pictures = [
      (&self.scope.objects, SCOPE, GREEN),
      (&self.response.objects, RESPONSE, CYAN),
      (&self.envelope.objects, ENVELOPE, AMBER),
    ];
    for (picture, (x, width), [red, green, blue, _]) in pictures {
      drawn.extend(picture.iter().cloned());
      lamps.extend(screen(width).lamps(x, SCREEN_Y, Vec3::new(red, green, blue), 0.8));
    }
    drawn.extend(self.rider.objects.iter().cloned());

    let scene = Scene {
      size: SIZE.into(),
      heights: [-8.0, 40.0],
      lean: LEAN,
      // dimmed, so that what the screens show stands out
      light: Light {
        color: Vec3::splat(0.7),
        ambient: Vec3::splat(0.25),
        ..Light::default()
      },
      look: Look::default(),
      objects: drawn.objects,
      lamps,
    };
    (scene, drawn.targets)
  }
}

impl Skin for Synth {
  fn prepare(&mut self, gpu: &Gpu, view: &View, over: &mut Flat) {
    self.dots = dots(STAGES.map(|stage| view.values[stage]));
    let (scene, targets) = self.scene(view.values, view.feed, view.held, view.notes);
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
    // the dots lie flat on their screen, so they are found by their places and the rest by
    // the scene
    if let Some(dot) = shared::dot_at(&self.dots, at, DOT_REACH) {
      return Some(Target::Control(ATTACK_DOT + dot));
    }
    // the place on the bed where the line of sight meets the white keys' tops
    let on_bed = Vec2::from(at) - KEYS - LEAN * self.keybed.height;
    if let Some(note) = self.keybed.note_at(on_bed) {
      let velocity = self.keybed.velocity_at(note, on_bed.y);
      return Some(Target::Key { note, velocity });
    }
    self.targets[self.renderer.pick(at.into())?]
  }
}

/// The stage of its envelope a note is in.
#[derive(Clone, Copy, PartialEq)]
enum Stage {
  Attack,
  Decay,
  Hold,
  Release,
}

/// The one note the thread plays at a time, and its envelope.
struct Voice {
  /// The note whose key is down.
  note: Option<u8>,
  /// The stage the envelope is in, if it has not run out.
  stage: Option<Stage>,
  /// The part of the stage's time that has passed.
  part: f32,
  /// The envelope's level at the start of the stage, and now.
  from: f32,
  level: f32,
}

impl Voice {
  fn enter(&mut self, stage: Stage) {
    (self.stage, self.part, self.from) = (Some(stage), 0.0, self.level);
  }

  /// Begins a note, in place of any that sounds.
  ///
  /// What the scope shows of it is written to `feed`.
  fn strike(&mut self, note: u8, velocity: u8, feed: &Feed) {
    self.note = Some(note);
    self.enter(Stage::Attack);
    feed.set(PITCH, f32::from(note));
    feed.set(STRUCK, f32::from(velocity) / 127.0);
  }

  /// Ends a note, if it is the one that sounds.
  fn lift(&mut self, note: u8) {
    if self.note == Some(note) {
      self.note = None;
      self.enter(Stage::Release);
    }
  }

  /// Takes the envelope on by the time of one block.
  ///
  /// The four are the plain values of [`STAGES`]: the times in milliseconds and the sustain
  /// in hundredths.
  fn advance(&mut self, [attack, decay, sustain, release]: [f32; 4]) {
    let Some(stage) = self.stage else {
      return;
    };
    let sustain = sustain / 100.0;
    let (to, seconds) = match stage {
      Stage::Attack => (1.0, attack / 1000.0),
      Stage::Decay => (sustain, decay / 1000.0),
      Stage::Hold => (sustain, HOLD),
      Stage::Release => (0.0, release / 1000.0),
    };
    self.part = (self.part + BLOCK.as_secs_f32() / seconds).min(1.0);
    // held, the level is the sustain wherever its fader goes
    self.level = if stage == Stage::Hold {
      sustain
    } else {
      self.from + (to - self.from) * eased(self.part)
    };
    if self.part < 1.0 {
      return;
    }
    match stage {
      Stage::Attack => self.enter(Stage::Decay),
      Stage::Decay => self.enter(Stage::Hold),
      // the rider waits at the end of the hold until the key comes up
      Stage::Hold => {}
      Stage::Release => self.stage = None,
    }
  }

  /// Returns how far along the envelope the note is.
  ///
  /// It is as [`ALONG`] has it.
  fn along(&self) -> f32 {
    self
      .stage
      .map_or(-1.0, |stage| stage as u8 as f32 + self.part)
  }
}

/// Returns the function that plays one block of the notes.
///
/// The notes are the panel's own and a made-up run of a host's. What is seen of them is written
/// to `feed`. The parameters are as `values` has them. A block
/// goes the way one of a plugin's audio thread does: it takes the notes the keys have played
/// since the last block, plays the run while its button is in, and takes the envelope on.
fn sound(feed: Feed, notes: Notes, values: Arc<[AtomicU32]>) -> impl FnMut() + Send {
  let mut heard = Heard::new(&notes);
  let mut voice = Voice {
    note: None,
    stage: None,
    part: 0.0,
    from: 0.0,
    level: 0.0,
  };
  // the block of the run that comes next, and the note of it that is down
  let (mut block, mut run_note) = (0, None);
  move || {
    notes.hear(&mut heard, |played| match played {
      Played::On { note, velocity } => voice.strike(note, velocity, &feed),
      Played::Off { note } => voice.lift(note),
    });

    // the run begins a note at the start of each step and ends it a little before the next.
    // the window draws the keys' own notes down itself, and is shown a host's here
    let playing = shared::value(&values, HOST) > 0.5;
    let (step, within) = (block / STEP, block % STEP);
    if (!playing || within == DOWN_FOR)
      && let Some(note) = run_note.take()
    {
      notes.show(note, false);
      voice.lift(note);
    }
    if playing && within == 0 {
      let note = RUN[step as usize % RUN.len()];
      notes.show(note, true);
      voice.strike(note, 100, &feed);
      run_note = Some(note);
    }
    block = if playing { block + 1 } else { 0 };

    let plain = |parameter: usize, axis: Axis| axis.plain(shared::value(&values, parameter));
    voice.advance([
      plain(ATTACK, ATTACKS),
      plain(DECAY, DECAYS),
      plain(SUSTAIN, SUSTAINS),
      plain(RELEASE, RELEASES),
    ]);
    feed.set(LEVEL, voice.level);
    feed.set(ALONG, voice.along());
  }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut settings = Settings::new("potpot synth", SIZE);
  let (feed, notes) = (Feed::new(SLOTS), Notes::new());
  feed.set(ALONG, -1.0);
  feed.set(PITCH, f32::from(REST));
  feed.set(STRUCK, 1.0);
  (settings.feed, settings.notes) = (feed.clone(), notes.clone());
  let (dials, controls) = wiring();
  let (params, values) = shared::params(dials);
  if shared::window::file().is_some() {
    // a picture has the run playing, as a press on its button leaves it
    shared::set(&values, HOST, 1.0);
  }
  let sound = shared::window::Sound {
    block: Box::new(sound(feed, notes, values)),
    every: BLOCK,
    pictured_after: PICTURED_AFTER,
  };
  shared::window::run(settings, params, controls, Some(sound), Synth::new)
}
