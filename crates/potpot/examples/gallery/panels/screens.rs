//! Screens that show an equalizer, a scope and an envelope.

use std::f32::consts::TAU;
use std::sync::Arc;

use potpot::glam::{Mat4, Vec2, Vec3};
use potpot::parts::{self, Knob, Screen};
use potpot::{Light, Look, Material, Object, Scene};
use potpot_controls::axis::Axis;
use potpot_controls::flat::Flat;
use potpot_controls::response::{self, Band, Kind};

use super::shared::{self, GRID, Plot};
use super::{BLACK, SIZE, framed, lettering, screws};

/// The sample rate the curve is drawn for, in hertz.
const RATE: f32 = 48000.0;

/// Adds an equalizer of three bands.
///
/// Its curve lies over a made-up spectrum, with a dot for each band.
fn equalizer(flat: &mut Flat, plot: Plot) {
  let (hertz, decibels) = (Axis::log([20.0, 20000.0]), Axis::linear([-18.0, 18.0]));
  let bands = [
    Band {
      kind: Kind::LowShelf,
      frequency: 120.0,
      gain: 7.0,
      q: 0.7,
    },
    Band {
      kind: Kind::Bell,
      frequency: 900.0,
      gain: -9.0,
      q: 1.4,
    },
    Band {
      kind: Kind::HighShelf,
      frequency: 6000.0,
      gain: 5.0,
      q: 0.7,
    },
  ];

  // what the sound might be: louder in the bass, with a few peaks, shaped by the bands
  let spectrum: Vec<[f32; 2]> = (0..=96)
    .map(|bin| {
      let across = f32::from(bin as u8) / 96.0;
      let frequency = hertz.plain(across);
      let peaks = (across * 31.0).sin() * (across * 83.0).cos() * 5.0;
      let shaped: f32 = bands.iter().map(|band| band.gain_at(frequency, RATE)).sum();
      plot.at([across, decibels.turn(-4.0 - across * 12.0 + peaks + shaped)])
    })
    .collect();
  flat.area(&spectrum, plot.max.y, [0.04, 0.2, 0.24, 1.0]);

  // a line for each decade and for each six decibels, and then the decades' names
  let named = [(100.0, "100"), (1000.0, "1K"), (10000.0, "10K")];
  for (frequency, _) in named {
    let x = hertz.turn(frequency);
    flat.line(plot.at([x, 0.0]), plot.at([x, 1.0]), 1.0, GRID);
  }
  for gain in [-12.0, -6.0, 0.0, 6.0, 12.0] {
    let y = decibels.turn(gain);
    flat.line(plot.at([0.0, y]), plot.at([1.0, y]), 1.0, GRID);
  }
  let ink = [0.25, 0.36, 0.4, 1.0];
  for (frequency, name) in named {
    let [x, y] = plot.at([hertz.turn(frequency), 0.0]);
    flat.legend(name, [x + 4.0, y - 10.0], 6.0, 1.0, ink);
  }

  let curve: Vec<[f32; 2]> = response::trace(&bands, RATE, hertz, decibels, 240)
    .into_iter()
    .map(|point| plot.at(point))
    .collect();
  flat.curve(&curve, 2.2, [0.45, 0.95, 1.0, 1.0]);
  for band in bands {
    let at = plot.at([hertz.turn(band.frequency), decibels.turn(band.gain)]);
    flat.line(at, at, 12.0, [1.0, 0.72, 0.2, 1.0]);
    flat.line(at, at, 6.0, [0.1, 0.06, 0.01, 1.0]);
  }
}

/// Adds a scope's trace of a note that dies away.
fn scope(flat: &mut Flat, plot: Plot) {
  plot.divide(flat, 8, 4);
  let trace: Vec<[f32; 2]> = (0..=300)
    .map(|step| {
      let time = step as f32 / 300.0;
      let wave = (time * TAU * 9.0).sin() + 0.35 * (time * TAU * 27.0 + 1.0).sin();
      plot.at([time, 0.5 + 0.34 * wave * (-time * 2.6).exp()])
    })
    .collect();
  flat.curve(&trace, 1.8, [0.4, 1.0, 0.5, 1.0]);
}

/// Adds an envelope of four stages.
///
/// The stages are a curve over the region under it.
fn envelope(flat: &mut Flat, plot: Plot) {
  plot.divide(flat, 8, 4);
  // where each stage ends across the plot, and how high: attack, decay, sustain, release
  let stages = [
    [0.0, 0.0],
    [0.14, 0.92],
    [0.42, 0.5],
    [0.7, 0.5],
    [1.0, 0.0],
  ];
  let mut curve = Vec::new();
  for stage in stages.windows(2) {
    let ([from, low], [to, high]) = (stage[0], stage[1]);
    for step in 0..24 {
      // each stage closes most of what is left of its way early on
      let part = step as f32 / 24.0;
      let eased = 1.0 - (1.0 - part).powi(3);
      curve.push(plot.at([from + (to - from) * part, low + (high - low) * eased]));
    }
  }
  curve.push(plot.at(stages[4]));
  flat.area(&curve, plot.max.y, [0.3, 0.16, 0.03, 1.0]);
  flat.curve(&curve, 2.0, [1.0, 0.7, 0.2, 1.0]);
  for stage in &stages[1..4] {
    let at = plot.at(*stage);
    flat.line(at, at, 7.0, [1.0, 0.9, 0.7, 1.0]);
  }
}

/// Returns three screens on a dark panel.
///
/// Under them is a row of knobs that their light falls on. The large one shows an equalizer's
/// curve, and the two beside it a scope's trace and an envelope. The light is dimmed so that what
/// the screens show stands out.
pub(super) fn screens(look: Look) -> Scene {
  let (width, height) = (SIZE[0] as f32, SIZE[1] as f32);
  let ink = Material::ink(Vec3::splat(0.8));
  let put = |mesh: potpot::Mesh, at: Vec2, material: Material| Object {
    mesh: Arc::new(mesh),
    transform: Mat4::from_translation(at.extend(0.0)),
    material,
  };
  let mut objects = vec![put(
    parts::plate(width, height, 6.0),
    Vec2::ZERO,
    Material::powder_coat(Vec3::new(0.075, 0.08, 0.09)),
  )];
  let mut lamps = Vec::new();

  let small = Screen::new(300.0, 130.0);
  type Picture = fn(&mut Flat, Plot);
  let shown: [(&str, Screen, Vec2, Vec3, Picture); 3] = [
    (
      "EQUALIZER",
      Screen::new(600.0, 300.0),
      Vec2::new(340.0, 215.0),
      Vec3::new(0.3, 0.9, 1.0),
      equalizer,
    ),
    (
      "SCOPE",
      small,
      Vec2::new(830.0, 130.0),
      Vec3::new(0.3, 1.0, 0.45),
      scope,
    ),
    (
      "ENVELOPE",
      small,
      Vec2::new(830.0, 300.0),
      Vec3::new(1.0, 0.65, 0.15),
      envelope,
    ),
  ];
  for (name, screen, middle, color, picture) in shown {
    let glass = shared::glass();
    objects.push(put(screen.face(), middle, glass));
    objects.push(put(screen.bezel(), middle, Material::glossy(BLACK)));
    let over = Vec2::new(0.0, -screen.length / 2.0 - screen.border - 12.0);
    objects.push(put(lettering(name, 7.0), middle + over, ink));
    let mut flat = Flat::default();
    picture(&mut flat, Plot::of(&screen, middle, 10.0));
    objects.extend(parts::picture(&flat, middle.x, middle.y, glass, 0));
    lamps.extend(screen.lamps(middle.x, middle.y, color, 1.0));
  }

  // knobs that show the screens' light on their sides and in their shine
  let knob = Knob::round(34.0, 30.0);
  let body = Arc::new(knob.body());
  let materials = [
    Material::chrome(Vec3::splat(0.8)),
    Material::glossy(BLACK),
    Material::satin(Vec3::new(0.74, 0.72, 0.66)),
  ];
  for index in 0..7 {
    let at = Vec2::new(100.0 + 137.0 * index as f32, 500.0);
    objects.push(Object {
      mesh: Arc::clone(&body),
      transform: Mat4::from_translation(at.extend(0.0)),
      material: materials[index % materials.len()],
    });
    let marks = parts::ticks(
      11,
      parts::KNOB_SWEEP,
      knob.radius + 6.0,
      knob.radius + 12.0,
      5,
    );
    objects.push(put(marks, at, ink));
  }

  objects.extend(screws());
  Scene {
    light: Light {
      color: Vec3::splat(0.6),
      ..Light::default()
    },
    lamps,
    ..framed(look, objects)
  }
}
