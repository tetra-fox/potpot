//! Rings of lights around a knob.

use std::f32::consts::FRAC_PI_2;
use std::sync::Arc;

use glam::{Mat4, Vec3};

use super::button::Button;
use super::led::{led, led_material, led_seat, led_seat_radius};
use super::outline::STRIP;
use super::{KNOB_SWEEP, PAINT, knob_transform, pipe_end, pipe_seat};
use crate::mesh::Mesh;
use crate::scene::Object;

/// A ring of lights around a knob, each the end of a light pipe.
///
/// The lights lie end to end around the origin over the sweep that
/// [`knob_transform`](super::knob_transform) turns a knob through, so each light stands for a
/// part of the knob's turn. [`LedRing::lights`] lights the run between two places of the turn.
/// A run from the start shows a level, a run from the middle a level either way, and a run from
/// where the knob points how far another value is from the knob's.
#[derive(Clone, Copy, Debug)]
pub struct LedRing {
  /// The count of lights.
  pub segments: u32,
  /// The angle the lights spread over, in radians.
  pub sweep: f32,
  /// The distance from the origin to the middle of each light, in points.
  pub radius: f32,
  /// The width of each light out from the origin, in points.
  pub width: f32,
  /// The share of its part of the ring a light takes.
  ///
  /// The gap to the next light takes what is left.
  pub fill: f32,
  /// The width of the dark gap around each light, in points.
  pub margin: f32,
}

impl LedRing {
  /// Creates a ring of lights over the turn of a knob.
  ///
  /// The ring has `segments` lights over [`KNOB_SWEEP`], with their middles `radius` from the
  /// origin, each `width` across.
  pub fn new(segments: u32, radius: f32, width: f32) -> Self {
    Self {
      segments,
      sweep: KNOB_SWEEP,
      radius,
      width,
      fill: 0.7,
      margin: 0.6,
    }
  }

  /// Returns the shape of one light pipe's end, lying across the ring.
  fn lens(&self) -> Button {
    let share = self.sweep / self.segments.max(1) as f32;
    let along = 2.0 * self.radius * (share * self.fill.clamp(0.0, 1.0) / 2.0).sin();
    Button {
      corner: 0.3,
      roll: 0.6,
      crown: 0.1,
      ..Button::key(along, self.width, self.width.min(along) * 0.22)
    }
  }

  /// Returns one light, lying on the origin.
  ///
  /// Every light of the ring is this mesh.
  pub fn segment(&self) -> Mesh {
    self.lens().body()
  }

  /// Returns the dark gap around one light set into the panel.
  pub fn seat(&self) -> Mesh {
    self.lens().seat(self.margin)
  }

  /// Returns the transform of the light at `index` of a ring around (`x`, `y`).
  fn place(&self, x: f32, y: f32, index: u32) -> Mat4 {
    let turn = (index as f32 + 0.5) / self.segments as f32;
    Mat4::from_translation(Vec3::new(x, y, 0.0))
      * Mat4::from_rotation_z((turn - 0.5) * self.sweep)
      * Mat4::from_translation(Vec3::new(0.0, -self.radius, 0.0))
  }

  /// Returns the gaps around every light as objects.
  ///
  /// Each [`Object`] has the mesh [`LedRing::seat`] makes, for a ring around (`x`, `y`).
  pub fn seats(&self, seat: &Arc<Mesh>, x: f32, y: f32) -> impl Iterator<Item = Object> {
    (0..self.segments).map(move |index| Object {
      mesh: Arc::clone(seat),
      transform: self.place(x, y, index),
      material: pipe_seat(),
    })
  }

  /// Returns how much of each light a run covers, from zero to one, in the order of the lights.
  ///
  /// The run is between the two places of `run`, in either order. Each place is a part of the
  /// sweep, from zero at its start to one at its end, and one past either end is at that end. A
  /// run of no length covers nothing.
  pub fn levels(&self, run: [f32; 2]) -> impl Iterator<Item = f32> {
    let count = self.segments as f32;
    let [from, to] = run.map(|place| place.clamp(0.0, 1.0) * count);
    let (low, high) = (from.min(to), from.max(to));
    (0..self.segments).map(move |index| {
      let start = index as f32;
      (high.min(start + 1.0) - low.max(start)).max(0.0)
    })
  }

  /// Returns the ring's lights as objects, each lit in `color` at its level of `levels`.
  ///
  /// Each [`Object`] has the mesh [`LedRing::segment`] makes, for a ring around (`x`, `y`).
  /// `levels` runs from zero for dark to one for lit, one for each light in turn, and a light
  /// past the end of it is dark.
  pub fn lit(
    &self,
    segment: &Arc<Mesh>,
    x: f32,
    y: f32,
    levels: impl IntoIterator<Item = f32>,
    color: Vec3,
  ) -> impl Iterator<Item = Object> {
    let mut levels = levels.into_iter();
    (0..self.segments).map(move |index| Object {
      mesh: Arc::clone(segment),
      transform: self.place(x, y, index),
      material: pipe_end(color, levels.next().unwrap_or(0.0), self.width * 0.6),
    })
  }

  /// Returns the ring's lights as objects, lit over a run.
  ///
  /// The lights are lit in `color` as much as [`LedRing::levels`] says `run` covers them, for a
  /// ring around (`x`, `y`) as [`LedRing::lit`] lays it out.
  pub fn lights(
    &self,
    segment: &Arc<Mesh>,
    x: f32,
    y: f32,
    run: [f32; 2],
    color: Vec3,
  ) -> impl Iterator<Item = Object> {
    self.lit(segment, x, y, self.levels(run), color)
  }
}

/// A small round light at each stop of a knob that clicks from stop to stop.
///
/// The stops spread evenly over the sweep that [`knob_transform`] turns a knob through, from one
/// end to the other, and a lone stop is at the middle. [`LedStops::lights`] lights the stop the
/// knob is at and leaves the others dim.
#[derive(Clone, Copy, Debug)]
pub struct LedStops {
  /// The count of stops.
  pub stops: u32,
  /// The angle the stops spread over, in radians.
  pub sweep: f32,
  /// The distance from the origin to the middle of each light, in points.
  pub radius: f32,
  /// The width of each light, in points.
  pub diameter: f32,
  /// How bright a light is at a stop the knob is not at, from zero for dark to one.
  pub dim: f32,
}

impl LedStops {
  /// Creates a light `diameter` wide at each of `stops` stops over [`KNOB_SWEEP`], with their
  /// middles `radius` from the origin.
  pub fn new(stops: u32, radius: f32, diameter: f32) -> Self {
    Self {
      stops,
      sweep: KNOB_SWEEP,
      radius,
      diameter,
      dim: 0.1,
    }
  }

  /// Returns the place of the sweep `stop` is at, from zero at its start to one at its end.
  pub fn turn(&self, stop: u32) -> f32 {
    match self.stops {
      0 | 1 => 0.5,
      stops => stop as f32 / (stops - 1) as f32,
    }
  }

  /// Returns how far the lights and their seats reach up from the origin, toward negative y,
  /// and how far down.
  pub fn reach(&self) -> [f32; 2] {
    let seat = led_seat_radius(self.diameter);
    let ys = (0..self.stops).map(|stop| self.place(0.0, 0.0, stop).w_axis.y);
    let (up, down) = ys.fold((f32::MIN, f32::MIN), |(up, down), y| {
      (up.max(-y), down.max(y))
    });
    [up + seat, down + seat]
  }

  /// Returns one light, lying on the origin.
  pub fn light(&self) -> Mesh {
    led(self.diameter)
  }

  /// Returns the dark ring one light sits in, lying on the origin.
  pub fn seat(&self) -> Mesh {
    led_seat(self.diameter)
  }

  /// Returns the transform of the light at `stop` of a knob at (`x`, `y`).
  fn place(&self, x: f32, y: f32, stop: u32) -> Mat4 {
    knob_transform(x, y, self.turn(stop), self.sweep)
      * Mat4::from_translation(Vec3::new(0.0, -self.radius, 0.0))
  }

  /// Returns the ring each light sits in, as objects.
  ///
  /// Each [`Object`] has the mesh [`LedStops::seat`] makes, for a knob at (`x`, `y`).
  pub fn seats(&self, seat: &Arc<Mesh>, x: f32, y: f32) -> impl Iterator<Item = Object> {
    (0..self.stops).map(move |stop| Object {
      mesh: Arc::clone(seat),
      transform: self.place(x, y, stop),
      material: pipe_seat(),
    })
  }

  /// Returns the lights as objects.
  ///
  /// Each [`Object`] has the mesh [`LedStops::light`] makes, for a knob at (`x`, `y`). The stop
  /// nearest `place` of the sweep is lit in `color` and the others are lit at [`LedStops::dim`].
  pub fn lights(
    &self,
    light: &Arc<Mesh>,
    x: f32,
    y: f32,
    place: f32,
    color: Vec3,
  ) -> impl Iterator<Item = Object> {
    let last = self.stops.saturating_sub(1);
    let at = (place.clamp(0.0, 1.0) * last as f32).round() as u32;
    (0..self.stops).map(move |stop| {
      let level = if stop == at { 1.0 } else { self.dim };
      Object {
        mesh: Arc::clone(light),
        transform: self.place(x, y, stop),
        material: led_material(color, level, self.diameter),
      }
    })
  }
}

/// An unbroken arc of light around a knob, behind one long lens.
///
/// The arc lies over the sweep that [`knob_transform`](super::knob_transform) turns a knob
/// through, like [`LedRing`], and [`LedArc::lights`] lights the run between two places of the
/// turn. The lit run ends where the run does, so the arc shows a value more finely than a ring
/// of separate lights.
#[derive(Clone, Copy, Debug)]
pub struct LedArc {
  /// The angle the arc spreads over, in radians.
  pub sweep: f32,
  /// The distance from the origin to the middle of the arc, in points.
  pub radius: f32,
  /// The width of the arc out from the origin, in points.
  pub width: f32,
  /// The width of the dark gap around the lens, in points.
  pub margin: f32,
}

impl LedArc {
  /// Creates an arc over the turn of a knob.
  ///
  /// The arc spans [`KNOB_SWEEP`] with its middle `radius` from the origin, and is `width` across.
  pub fn new(radius: f32, width: f32) -> Self {
    Self {
      sweep: KNOB_SWEEP,
      radius,
      width,
      margin: 0.6,
    }
  }

  /// Returns a flat strip over a run of the arc, `grown` wider to each side and `height` high.
  ///
  /// `run` is two places of the sweep as [`LedArc::lights`] takes them, in order.
  fn strip(&self, [from, to]: [f32; 2], grown: f32, height: f32) -> Mesh {
    // the place at the middle of the sweep is where a knob points before it turns, which is
    // toward negative y
    let angle = |place: f32| (place - 0.5) * self.sweep - FRAC_PI_2;
    // about one side to every two degrees
    let sides = ((to - from) * self.sweep.to_degrees() / 2.0)
      .ceil()
      .max(1.0) as u32;
    let half = self.width / 2.0 + grown;
    Mesh::lathe_between(
      &[[self.radius + half, height], [self.radius - half, height]],
      [angle(from), angle(to)],
      sides,
      true,
    )
  }

  /// Returns the dark gap the lens is set in, over the whole sweep.
  pub fn seat(&self) -> Mesh {
    self.strip([0.0, 1.0], self.margin, STRIP)
  }

  /// Returns the lens over the whole sweep, unlit.
  pub fn lens(&self) -> Mesh {
    self.strip([0.0, 1.0], 0.0, PAINT)
  }

  /// Returns the arc's seat, its lens and the run of it that is lit, as objects.
  ///
  /// `seat` and `lens` are the meshes [`LedArc::seat`] and [`LedArc::lens`] make, for an arc
  /// around (`x`, `y`). The run lit in `color` is between the two places of `run`, in either
  /// order. Each place is a part of the sweep, from zero at its start to one at its end, and one
  /// past either end is at that end. A run of no length lights nothing, and leaves only the dark
  /// lens. The lit run's mesh is made anew on each call.
  pub fn lights(
    &self,
    [seat, lens]: &[Arc<Mesh>; 2],
    x: f32,
    y: f32,
    run: [f32; 2],
    color: Vec3,
  ) -> impl Iterator<Item = Object> {
    let on_panel = Mat4::from_translation(Vec3::new(x, y, 0.0));
    let [from, to] = run.map(|place| place.clamp(0.0, 1.0));
    let (low, high) = (from.min(to), from.max(to));
    let seat = Object {
      mesh: Arc::clone(seat),
      transform: on_panel,
      material: pipe_seat(),
    };
    let lens = Object {
      mesh: Arc::clone(lens),
      transform: on_panel,
      material: pipe_end(color, 0.0, 0.0),
    };
    // just over the lens, so that the lit run covers it
    let lit = (high > low).then(|| Object {
      mesh: Arc::new(self.strip([low, high], 0.0, PAINT * 2.0)),
      transform: on_panel,
      material: pipe_end(color, 1.0, 0.0),
    });
    [seat, lens].into_iter().chain(lit)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_ring_lights_the_run_between_two_places_either_way_round() {
    let ring = LedRing {
      sweep: 2.0 * FRAC_PI_2,
      ..LedRing::new(10, 40.0, 4.0)
    };
    let segment = Arc::new(ring.segment());
    let glows = |run| -> Vec<f32> {
      (ring.lights(&segment, 0.0, 0.0, run, Vec3::ONE))
        .map(|light| light.material.glow)
        .collect()
    };
    // from the middle down to a fifth, which is three whole lights
    let down = glows([0.5, 0.2]);
    assert_eq!(down, glows([0.2, 0.5]));
    assert_eq!(&down[..2], &[0.0; 2]);
    assert!((down[2] - 1.0).abs() < 1e-5 && (down[4] - 1.0).abs() < 1e-5);
    assert_eq!(&down[5..], &[0.0; 5]);
    // and down to a quarter, which covers half the third
    let part = glows([0.5, 0.25]);
    assert!((part[2] - 0.5).abs() < 1e-5, "{}", part[2]);
    // a run past an end stops there, and a run of no length lights nothing
    assert_eq!(glows([0.95, 1.6])[9], 0.5);
    assert!(glows([0.37, 0.37]).iter().all(|&glow| glow == 0.0));
    // the first light is a little way into the sweep from its start, which is to the left
    // of the middle here, and every one is the radius out
    let lights: Vec<Object> = ring
      .lights(&segment, 0.0, 0.0, [0.0, 0.0], Vec3::ONE)
      .collect();
    let first = lights[0].transform.transform_point3(Vec3::ZERO);
    let angle = (0.05 - 0.5) * ring.sweep;
    assert!((first - Vec3::new(40.0 * angle.sin(), -40.0 * angle.cos(), 0.0)).length() < 1e-4);
    assert!(first.x < 0.0);
    for light in &lights {
      let middle = light.transform.transform_point3(Vec3::ZERO);
      assert!((middle.length() - 40.0).abs() < 1e-4);
    }
  }

  #[test]
  fn neighboring_lights_of_a_ring_leave_the_gap_their_fill_leaves() {
    let ring = LedRing::new(24, 50.0, 4.0);
    let segment = ring.segment();
    let along = |vertex: &crate::Vertex| vertex.position[0];
    let half = (segment.vertices.iter()).map(along).fold(0.0, f32::max);
    let share = ring.sweep / 24.0;
    // a light is the chord of its share of the ring, cut to the fill
    let chord = 2.0 * 50.0 * (share * ring.fill / 2.0).sin();
    assert!(
      (half * 2.0 - chord).abs() < 0.05,
      "{} for {chord}",
      half * 2.0
    );
    // and the middles of two neighbors are the chord of a whole share apart
    let lights: Vec<Vec3> = (ring.lights(&Arc::new(segment), 0.0, 0.0, [0.0, 0.0], Vec3::ONE))
      .map(|light| light.transform.transform_point3(Vec3::ZERO))
      .collect();
    let apart = 2.0 * 50.0 * (share / 2.0).sin();
    for pair in lights.windows(2) {
      assert!((pair[0].distance(pair[1]) - apart).abs() < 1e-3);
    }
    assert!(apart - chord > 1.0, "the gap is {}", apart - chord);
  }

  #[test]
  fn an_arc_lights_from_its_start_middle_or_end() {
    let arc = LedArc::new(40.0, 3.0);
    let meshes = [Arc::new(arc.seat()), Arc::new(arc.lens())];
    // the least and greatest turn of the lit run from negative y, which is where a knob
    // points in the middle of its sweep. a knob turns counterclockwise
    let lit = |run| {
      let objects: Vec<Object> = arc.lights(&meshes, 0.0, 0.0, run, Vec3::ONE).collect();
      let lit = objects.get(2)?;
      let angles: Vec<f32> = (lit.mesh.vertices.iter())
        .map(|vertex| {
          let position = Vec3::from(vertex.position);
          assert!((position.truncate().length() - 40.0).abs() <= 1.5 + 1e-4);
          position.x.atan2(-position.y)
        })
        .collect();
      let low = angles.iter().copied().fold(f32::MAX, f32::min);
      let high = angles.iter().copied().fold(f32::MIN, f32::max);
      Some([low, high])
    };
    let half = arc.sweep / 2.0;
    let close = |[low, high]: [f32; 2], [want_low, want_high]: [f32; 2]| {
      (low - want_low).abs() < 1e-4 && (high - want_high).abs() < 1e-4
    };
    let up = lit([0.5, 1.0]).expect("a run to the end lights");
    assert!(close(up, [0.0, half]), "{up:?}");
    let down = lit([0.5, 0.0]).expect("a run to the start lights");
    assert!(close(down, [-half, 0.0]), "{down:?}");
    let all = lit([-1.0, 2.0]).expect("a run past both ends lights the whole arc");
    assert!(close(all, [-half, half]), "{all:?}");
    // at the start, the middle or the end a run of no length lights nothing
    for place in [0.0, 0.5, 1.0] {
      assert!(lit([place, place]).is_none(), "{place}");
    }
  }

  #[test]
  fn the_stops_of_a_knob_spread_from_end_to_end() {
    let lit = |stops: &LedStops, place: f32| -> Vec<f32> {
      let light = Arc::new(stops.light());
      (stops.lights(&light, 0.0, 0.0, place, Vec3::ONE))
        .map(|light| light.material.glow)
        .collect()
    };
    let middle = |stops: &LedStops, stop: u32| -> Vec3 {
      stops.place(0.0, 0.0, stop).transform_point3(Vec3::ZERO)
    };
    // a lone stop is straight up, and lit wherever the knob is
    let one = LedStops::new(1, 20.0, 4.0);
    assert!((middle(&one, 0) - Vec3::new(0.0, -20.0, 0.0)).length() < 1e-4);
    assert_eq!(lit(&one, 0.9), [1.0]);
    // two are at the ends of the sweep, either side of straight up and as far down
    let two = LedStops::new(2, 20.0, 4.0);
    let [left, right] = [0, 1].map(|stop| middle(&two, stop));
    let half = two.sweep / 2.0;
    assert!((left - Vec3::new(-20.0 * half.sin(), -20.0 * half.cos(), 0.0)).length() < 1e-4);
    assert!((right - Vec3::new(-left.x, left.y, 0.0)).length() < 1e-4);
    assert_eq!(lit(&two, 0.4), [1.0, two.dim]);
    assert_eq!(lit(&two, 0.6), [two.dim, 1.0]);
    // many are evenly apart, and the one nearest the place is lit
    let five = LedStops::new(5, 20.0, 4.0);
    let middles: Vec<Vec3> = (0..5).map(|stop| middle(&five, stop)).collect();
    let apart = middles[0].distance(middles[1]);
    for pair in middles.windows(2) {
      assert!((pair[0].distance(pair[1]) - apart).abs() < 1e-4);
    }
    assert!((middles[2] - Vec3::new(0.0, -20.0, 0.0)).length() < 1e-4);
    assert_eq!(lit(&five, 0.3)[1], 1.0);
    assert_eq!(lit(&five, 2.0)[4], 1.0);
    // the seats reach as far up and down as the outermost seat does. the ends of the sweep
    // are below the middle, so two stops reach less far up than down
    let ends = -20.0 * half.cos();
    for (stops, want) in [
      (one, [20.0, -20.0]),
      (two, [-ends, ends]),
      (five, [20.0, ends]),
    ] {
      let seat = stops.seat();
      let radius = (seat.vertices.iter())
        .map(|vertex| Vec3::from(vertex.position).truncate().length())
        .fold(0.0, f32::max);
      let [up, down] = stops.reach();
      assert!(
        (up - want[0] - radius).abs() < 1e-3,
        "{stops:?} reaches up {up}"
      );
      assert!(
        (down - want[1] - radius).abs() < 1e-3,
        "{stops:?} reaches down {down}"
      );
    }
  }
}
