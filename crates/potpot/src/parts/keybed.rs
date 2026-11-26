//! Piano keybeds.

use std::f32::consts::FRAC_PI_2;
use std::ops::RangeInclusive;

use glam::{Mat4, Vec2, Vec3};

use super::CREASE;
use super::outline::Outline;
use crate::mesh::Mesh;

/// The count of steps in the roll of a key's front edge.
const ROLL: usize = 4;

/// A piano keybed: white keys with black keys over them.
///
/// The bed is centered on the origin. Its notes run along x from the lowest at the left, and a
/// key reaches from its back edge, up the panel, to its front edge down it. The bed is as wide
/// as its notes make it, which [`Keybed::width`] returns.
///
/// A note is a MIDI note number from 0 to 127, of which 60 is middle C. The bed begins and
/// ends on any note: a black key at an end stands out past the last white key, with nothing in
/// front of it.
///
/// The backs of the keys from C to E share the width of three white keys evenly, and those from
/// F to B the width of four, which puts each black key off the middle of the gap under it as a
/// piano has it.
#[derive(Clone, Copy, Debug)]
pub struct Keybed {
  /// The lowest note, at most `last`.
  first: u8,
  /// The highest note, at most 127.
  last: u8,
  /// The distance from one white key to the next, in points.
  pub pitch: f32,
  /// The length of a white key along y, in points.
  pub length: f32,
  /// The height of a white key's top, in points.
  pub height: f32,
  /// The gap between two white keys, in points.
  pub gap: f32,
  /// The width of a black key at its foot, in points.
  pub black_width: f32,
  /// The length of a black key along y, in points.
  ///
  /// It reaches that far from the back of the bed.
  pub black_length: f32,
  /// The height of a black key's top, in points.
  pub black_height: f32,
  /// The drop of a held white key's front, in points.
  pub travel: f32,
}

impl Keybed {
  /// Creates a keybed of a range of notes.
  ///
  /// Its white keys are `pitch` apart and `length` long.
  ///
  /// A white key is half as high as the pitch and goes down four fifths of that. A black key
  /// is nine tenths as high as the pitch, and reaches three fifths of the way to the front.
  ///
  /// # Panics
  ///
  /// Panics if `notes` is empty or ends past 127.
  pub fn new(notes: RangeInclusive<u8>, pitch: f32, length: f32) -> Self {
    assert!(
      !notes.is_empty() && *notes.end() <= 127,
      "a keybed has a note at least and none past 127"
    );
    Self {
      first: *notes.start(),
      last: *notes.end(),
      pitch,
      length,
      height: pitch * 0.5,
      gap: pitch * 0.06,
      black_width: pitch * 0.58,
      black_length: length * 0.6,
      black_height: pitch * 0.9,
      travel: pitch * 0.4,
    }
  }

  /// Returns the notes of the bed, lowest to highest.
  pub fn notes(&self) -> RangeInclusive<u8> {
    self.first..=self.last
  }

  /// Returns whether a note's key is a black one.
  pub fn is_black(note: u8) -> bool {
    matches!(note % 12, 1 | 3 | 6 | 8 | 10)
  }

  /// Returns the left and right edge of the bed.
  ///
  /// They are in points right of the left edge of note 0.
  fn edges(&self) -> [f32; 2] {
    // a black key at an end reaches past the white one beside it, unless it is a narrow one
    let ends = |notes: [u8; 2]| notes.map(|note| self.reach(note.clamp(self.first, self.last)));
    let [first, second] = ends([self.first, self.first.saturating_add(1)]);
    let [last, before] = ends([self.last, self.last.saturating_sub(1)]);
    [first[0].min(second[0]), last[1].max(before[1])]
  }

  /// Returns the left and right edge of a key.
  ///
  /// They are in points right of the left edge of note 0. A white key has half the gap to each
  /// side of it as well.
  fn reach(&self, note: u8) -> [f32; 2] {
    let (octave, tone) = (f32::from(note / 12), f32::from(note % 12));
    // how many white keys right of its octave's C the key's middle is
    let within = match (note % 12, Self::is_black(note)) {
      (0..=4, false) => (tone / 2.0).floor() + 0.5,
      (0..=4, true) => (tone + 0.5) * 3.0 / 5.0,
      (_, false) => 3.0 + ((tone - 5.0) / 2.0).floor() + 0.5,
      (_, true) => 3.0 + (tone - 4.5) * 4.0 / 7.0,
    };
    let middle = (octave * 7.0 + within) * self.pitch;
    let half = if Self::is_black(note) {
      self.black_width / 2.0
    } else {
      self.pitch / 2.0
    };
    [middle - half, middle + half]
  }

  /// Returns the bed's width along x, in points.
  pub fn width(&self) -> f32 {
    let [left, right] = self.edges();
    right - left
  }

  /// Returns how far right of the bed's middle a key's is.
  fn middle(&self, note: u8) -> f32 {
    let ([left, right], [from, to]) = (self.edges(), self.reach(note));
    (from + to) / 2.0 - (left + right) / 2.0
  }

  /// Returns the mesh of a white key.
  ///
  /// The key stands with its middle on the origin, and its front edge is rolled over.
  pub fn white(&self) -> Mesh {
    let (half, tall) = (self.length / 2.0, self.height);
    let roll = (tall * 0.14).min(half * 0.5);
    let mut outline = vec![Vec2::new(-half, 0.0), Vec2::new(-half, tall)];
    outline.extend((0..=ROLL).map(|step| {
      let (out, up) = (step as f32 / ROLL as f32 * FRAC_PI_2).sin_cos();
      Vec2::new(half - roll * (1.0 - out), tall - roll * (1.0 - up))
    }));
    outline.push(Vec2::new(half, 0.0));
    let across = (self.pitch - self.gap) / 2.0;
    Mesh::extruded(&outline, |_| across, CREASE)
  }

  /// Returns the mesh of a black key.
  ///
  /// The key stands with its middle on the origin. It narrows toward its top, and its front
  /// slants back from where it meets the white keys' tops.
  pub fn black(&self) -> Mesh {
    let (half, tall) = (self.black_length / 2.0, self.black_height);
    let over = (tall - self.height).max(0.0);
    let cut = (over * 0.12).min(half * 0.2);
    let slant = (over * 0.45).min(half);
    let outline = [
      Vec2::new(-half, 0.0),
      Vec2::new(-half, tall - cut),
      Vec2::new(-half + cut, tall),
      Vec2::new(half - slant, tall),
      Vec2::new(half, tall - over),
      Vec2::new(half, 0.0),
    ];
    let foot = self.black_width / 2.0;
    Mesh::extruded(
      &outline,
      |height| foot * (1.0 - 0.16 * height / tall),
      CREASE,
    )
  }

  /// Returns the gap under and around the keys as a dark strip.
  ///
  /// The strip lies on the panel, `gap` wider than the bed all around.
  pub fn seat(&self, gap: f32) -> Mesh {
    Outline::new(self.width(), self.length, 0.0)
      .grown(gap)
      .strip()
  }

  /// Returns the transform of a key of a bed at (`x`, `y`).
  ///
  /// `pressed` runs from zero for up to one for all the way down. A key tips about an axle
  /// that lies across the bed, one white key's length behind its back edge, so all of the key
  /// sinks and its front the most. All the way down, the front of a white key is
  /// [`Keybed::travel`] lower and its back half that.
  pub fn transform(&self, x: f32, y: f32, note: u8, pressed: f32) -> Mat4 {
    let (length, height) = if Self::is_black(note) {
      (self.black_length, self.black_height)
    } else {
      (self.length, self.height)
    };
    let axle = y - self.length * 1.5;
    let dip = (self.travel * pressed.clamp(0.0, 1.0) / (self.length * 2.0)).clamp(0.0, 1.0);
    // down the panel is y and up off it is z, so the front goes down as the turn goes back
    Mat4::from_translation(Vec3::new(x + self.middle(note), axle, height))
      * Mat4::from_rotation_x(-dip.asin())
      * Mat4::from_translation(Vec3::new(0.0, self.length + length / 2.0, -height))
  }

  /// Returns the note of the key at a point of the bed.
  ///
  /// The bed is seen from straight above. `at` is in points from the bed's middle. A black key is
  /// over the white ones as far as it is long, and a point in the gap between two white keys is on
  /// the nearer. In a scene that leans, the key drawn at a point of the panel is the one at that
  /// point less [`Scene::lean`](crate::Scene::lean) times [`Keybed::height`], which is where the
  /// line of sight meets the white keys' tops.
  pub fn note_at(&self, at: Vec2) -> Option<u8> {
    let [left, right] = self.edges();
    let (across, down) = (at.x + (left + right) / 2.0, at.y + self.length / 2.0);
    if !(0.0..=self.length).contains(&down) {
      return None;
    }
    let on = |note: &u8| {
      let [from, to] = self.reach(*note);
      (from..=to).contains(&across)
    };
    let of = |black: bool| {
      let mut keys = (self.first..=self.last).filter(move |&note| Self::is_black(note) == black);
      keys.find(on)
    };
    of(true)
      .filter(|_| down <= self.black_length)
      .or_else(|| of(false))
  }

  /// Returns the velocity of a press on a key.
  ///
  /// `y` is how far down the panel from the bed's middle the press is, in points. The velocity
  /// runs evenly from 1 at the back edge of the key to 127 at its front edge, and a press past
  /// either edge counts as on it.
  pub fn velocity_at(&self, note: u8, y: f32) -> u8 {
    let length = if Self::is_black(note) {
      self.black_length
    } else {
      self.length
    };
    let along = ((y + self.length / 2.0) / length).clamp(0.0, 1.0);
    1 + (along * 126.0).round() as u8
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// The lowest C of the beds the tests lay out.
  ///
  /// The notes over it are after it, by their names.
  const C: u8 = 48;
  const C_SHARP: u8 = C + 1;
  const D: u8 = C + 2;
  const E: u8 = C + 4;
  const F: u8 = C + 5;
  const F_SHARP: u8 = C + 6;
  const A_SHARP: u8 = C + 10;
  const B: u8 = C + 11;

  /// Returns a bed with white keys 20 apart and 100 long.
  fn bed(notes: RangeInclusive<u8>) -> Keybed {
    Keybed::new(notes, 20.0, 100.0)
  }

  /// Returns whether two distances are as good as the same.
  fn near(one: f32, other: f32) -> bool {
    (one - other).abs() < 1e-3
  }

  #[test]
  fn a_bed_is_as_wide_as_its_notes_from_white_or_black_ends() {
    // two octaves and a C: fifteen white keys, the first with its middle half a key in
    let whole = bed(C..=C + 24);
    assert!(near(whole.width(), 300.0), "{}", whole.width());
    assert!(near(whole.middle(C), -140.0));
    assert!(near(whole.middle(C + 24), 140.0));
    // each black key is off the gap under it: toward the outside of its group, and the middle
    // one of three right on it
    let off = |note: u8, gap: f32| whole.middle(note) + 150.0 - gap;
    let offs = [(1, 20.0), (3, 40.0), (6, 80.0), (8, 100.0), (10, 120.0)];
    let offs = offs.map(|(tone, gap)| off(C + tone, gap));
    for (off, expected) in offs
      .into_iter()
      .zip([-2.0, 2.0, -20.0 / 7.0, 0.0, 20.0 / 7.0])
    {
      assert!(near(off, expected), "{off} for {expected}");
    }
    // the same keys are as far apart an octave up, and E and F have no black key between them
    assert!(near(
      whole.middle(C_SHARP + 12) - whole.middle(C_SHARP),
      140.0
    ));
    assert!(near(whole.middle(F) - whole.middle(E), 20.0));

    // from a black key to a black key: each end is the edge of its black key, which stands
    // out past the nearest white one
    let black = bed(C_SHARP..=A_SHARP);
    let (first, last) = (
      black.black_width / 2.0 + 2.0,
      black.black_width / 2.0 + 20.0 / 7.0,
    );
    assert!(
      near(black.width(), first + 100.0 + last),
      "{}",
      black.width()
    );
    assert!(near(
      black.middle(C_SHARP),
      -black.width() / 2.0 + black.black_width / 2.0
    ));
    assert!(near(black.middle(D) - black.middle(C_SHARP), 12.0));
    assert!(near(
      black.middle(A_SHARP),
      black.width() / 2.0 - black.black_width / 2.0
    ));

    // from a white key to a black one and the other way, and a bed of one key of each kind
    assert!(near(
      bed(C..=C_SHARP).width(),
      18.0 + black.black_width / 2.0
    ));
    assert!(near(
      bed(A_SHARP..=B).width(),
      20.0 - 20.0 / 7.0 + black.black_width / 2.0
    ));
    assert!(near(bed(E..=F).width(), 40.0));
    assert!(near(bed(C..=C).width(), 20.0));
    assert!(near(bed(F_SHARP..=F_SHARP).width(), black.black_width));
    // a black key narrower than it is off its gap ends inside the white key beside it
    let narrow = Keybed {
      black_width: 2.0,
      ..bed(A_SHARP..=B)
    };
    assert!(near(narrow.width(), 20.0));
    // the highest notes there are
    assert!(near(bed(120..=127).width(), 100.0));
  }

  #[test]
  fn the_note_at_a_point_is_the_black_key_over_the_white_and_the_nearer_across_a_gap() {
    let keys = bed(C..=B);
    assert!(near(keys.width(), 140.0));
    // from the bed's top left corner
    let at = |x: f32, y: f32| keys.note_at(Vec2::new(x - 70.0, y - 50.0));
    assert_eq!(at(10.0, 90.0), Some(C));
    assert_eq!(at(30.0, 90.0), Some(D));
    assert_eq!(at(130.0, 5.0), Some(B));
    // the black key is over both white ones beside it at the back, 60 points down, and not in
    // front of itself
    assert_eq!(at(18.0, 59.0), Some(C_SHARP));
    assert_eq!(at(14.0, 30.0), Some(C_SHARP));
    assert_eq!(at(23.0, 30.0), Some(C_SHARP));
    assert_eq!(at(18.0, 61.0), Some(C));
    assert_eq!(at(23.0, 61.0), Some(D));
    // beside it at the back the white key shows
    assert_eq!(at(5.0, 30.0), Some(C));
    assert_eq!(at(25.0, 30.0), Some(D));
    // the gap between two white keys, a hair to each side of its middle
    assert_eq!(at(59.9, 90.0), Some(E));
    assert_eq!(at(60.1, 90.0), Some(F));
    assert_eq!(at(59.9, 10.0), Some(E));
    // off the bed on every side
    for (x, y) in [(-0.5, 50.0), (140.5, 50.0), (70.0, -0.5), (70.0, 100.5)] {
      assert_eq!(at(x, y), None, "{x}, {y}");
    }

    // a bed that ends on black keys has nothing in front of them
    let ends = bed(C_SHARP..=A_SHARP);
    let reach = ends.width() / 2.0;
    assert_eq!(ends.note_at(Vec2::new(-reach + 1.0, -40.0)), Some(C_SHARP));
    assert_eq!(ends.note_at(Vec2::new(-reach + 1.0, 40.0)), None);
    assert_eq!(ends.note_at(Vec2::new(reach - 1.0, -40.0)), Some(A_SHARP));
    assert_eq!(ends.note_at(Vec2::new(reach - 1.0, 40.0)), None);
    assert_eq!(ends.note_at(Vec2::new(-reach + 12.0, 40.0)), Some(D));
  }

  #[test]
  fn a_key_is_struck_harder_toward_its_front_and_never_with_nothing() {
    let keys = bed(C..=B);
    // a white key over the whole length of the bed, and a black one over its own
    assert_eq!(keys.velocity_at(C, -50.0), 1);
    assert_eq!(keys.velocity_at(C, 0.0), 64);
    assert_eq!(keys.velocity_at(C, 50.0), 127);
    assert_eq!(keys.velocity_at(C_SHARP, -50.0), 1);
    assert_eq!(keys.velocity_at(C_SHARP, 10.0), 127);
    // past either edge is the edge
    for note in [C, C_SHARP] {
      assert_eq!(keys.velocity_at(note, -400.0), 1);
      assert_eq!(keys.velocity_at(note, 400.0), 127);
    }
    let mut last = 0;
    for step in 0..=200 {
      let velocity = keys.velocity_at(C, -50.0 + step as f32 * 0.5);
      assert!((1..=127).contains(&velocity) && velocity >= last);
      last = velocity;
    }
  }

  #[test]
  fn a_held_key_sinks_about_an_axle_behind_the_bed_and_its_front_the_most() {
    let keys = bed(C..=B);
    let (white, black) = (keys.white(), keys.black());
    for (note, mesh, length, height) in [
      (D, &white, 100.0, keys.height),
      (C_SHARP, &black, keys.black_length, keys.black_height),
    ] {
      let (mut min, mut max) = (Vec3::MAX, Vec3::MIN);
      for vertex in &mesh.vertices {
        let position = Vec3::from(vertex.position);
        (min, max) = (min.min(position), max.max(position));
        assert!(Vec3::from(vertex.normal).is_normalized(), "{position}");
      }
      let width = if Keybed::is_black(note) {
        keys.black_width
      } else {
        keys.pitch - keys.gap
      };
      let half = Vec3::new(width / 2.0, length / 2.0, 0.0);
      assert!((min + half).length() < 1e-3, "{min}");
      assert!((max - half - Vec3::Z * height).length() < 1e-3, "{max}");

      // up, it stands with its back on the back of the bed
      let middle = 300.0 + keys.middle(note);
      let up = keys.transform(300.0, 400.0, note, 0.0);
      let back = Vec3::new(0.0, -length / 2.0, height);
      let front = Vec3::new(0.0, length / 2.0, height);
      assert!((up.transform_point3(back) - Vec3::new(middle, 350.0, height)).length() < 1e-3);
      // held, each end of its top is lower by its share of the travel, which the front of a
      // white key has all of, 200 points from the axle
      let down = keys.transform(300.0, 400.0, note, 1.0);
      let sunk = down.transform_point3(back);
      assert!(near(sunk.z, height - keys.travel / 2.0), "{sunk}");
      assert!(near(sunk.x, middle) && (sunk.y - 350.0).abs() < 0.1);
      let dipped = down.transform_point3(front);
      let share = (100.0 + length) / 200.0;
      assert!(near(dipped.z, height - keys.travel * share), "{dipped}");
      // and half held, half as much
      let halfway = keys.transform(300.0, 400.0, note, 0.5);
      let halfway = halfway.transform_point3(front);
      assert!(
        near(halfway.z, height - keys.travel * share / 2.0),
        "{halfway}"
      );
    }
    // a held black key is still over the white ones
    let share = (keys.length + keys.black_length) / (keys.length * 2.0);
    assert!(keys.black_height - keys.travel * share > keys.height);
  }
}
