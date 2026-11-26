//! Notes that pass between an open window and an audio thread.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// The count of notes there are, from 0 to 127.
const NOTES: usize = 128;
/// The bits of a slot that hold its last press's velocity.
const VELOCITY: u32 = 0xff;
/// The bit of a slot that is set while its note is down.
const DOWN: u32 = 1 << 8;
/// The lowest bit of a slot's count of presses.
///
/// The count takes the bits from here up, and goes around once they are full.
const PRESS: u32 = 1 << 9;

/// The notes of a keybed, shared with an audio thread.
///
/// A note is a MIDI note number from 0 to 127. The opener makes one, puts a clone in
/// [`Settings::notes`](crate::Settings::notes) and keeps another for the audio thread. Every
/// clone is the same notes.
///
/// The notes go both ways:
///
/// - The window writes each note that a key of the panel plays, a
///   [`Target::Key`](potpot_controls::interaction::Target::Key) of the skin. The audio thread
///   takes them with [`Notes::hear`], once a block. A note the panel holds comes up when the
///   window goes away, however it goes.
/// - The audio thread tells of each note the host plays with [`Notes::show`]. The window hands
///   the skin those and the panel's own as [`View::notes`](crate::View::notes), for it to draw
///   their keys down.
///
/// Nothing tells the window of a [`Notes::show`]. It looks at each of its ticks, and while a
/// shown note is down at each of its frames.
#[derive(Clone, Debug)]
pub struct Notes {
  slots: Arc<Slots>,
}

#[derive(Debug)]
struct Slots {
  /// Each note as the panel plays it.
  ///
  /// A slot holds the note's velocity, whether it is down and its count of presses.
  played: [AtomicU32; NOTES],
  /// A bit for each note the audio thread shows as down.
  ///
  /// The lower 64 notes are in the first.
  shown: [AtomicU64; 2],
}

impl Default for Notes {
  fn default() -> Self {
    Self::new()
  }
}

impl Notes {
  /// Creates the notes, each of them up.
  pub fn new() -> Self {
    Self {
      slots: Arc::new(Slots {
        played: std::array::from_fn(|_| AtomicU32::new(0)),
        shown: std::array::from_fn(|_| AtomicU64::new(0)),
      }),
    }
  }

  /// Tells of each note the panel played since the last call.
  ///
  /// `heard` is what the thread heard at its last call. `each` is called first for the end of
  /// every note that was down then and has come up since, lowest first, whether or not it is
  /// down again. Then it is called for every beginning, lowest first. A note that went down
  /// and came up again between two calls is told as a beginning with its end straight after.
  /// So a slide from one key to another is an end and then a beginning, whichever way it went.
  /// However often a note went down between two calls, it is told as having done so once.
  ///
  /// This is 128 atomic loads and the calls of `each`. It neither allocates, locks nor calls
  /// into the system, so an audio thread can call it.
  pub fn hear(&self, heard: &mut Heard, mut each: impl FnMut(Played)) {
    // each slot stands alone and nothing is ordered by it
    let now: [u32; NOTES] =
      std::array::from_fn(|note| self.slots.played[note].load(Ordering::Relaxed));
    let changes = || (0..).zip(heard.0.iter().zip(&now));
    // a press since the last look, which a note that is down again was let go for
    let pressed = |was: u32, now: u32| now & !(DOWN | VELOCITY) != was & !(DOWN | VELOCITY);
    for (note, (&was, &now)) in changes() {
      if was & DOWN != 0 && (pressed(was, now) || now & DOWN == 0) {
        each(Played::Off { note });
      }
    }
    for (note, (&was, &now)) in changes() {
      let down = now & DOWN != 0;
      if pressed(was, now) || (down && was & DOWN == 0) {
        let velocity = (now & VELOCITY) as u8;
        each(Played::On { note, velocity });
        if !down {
          each(Played::Off { note });
        }
      }
    }
    heard.0 = now;
  }

  /// Shows a note the host plays as down or as up.
  ///
  /// A note over 127 is none of the notes, and is left out.
  ///
  /// This is one atomic operation. It neither allocates, locks nor calls into the system, so an
  /// audio thread can call it.
  pub fn show(&self, note: u8, down: bool) {
    let Some(word) = self.slots.shown.get(usize::from(note / 64)) else {
      return;
    };
    let bit = 1 << (note % 64);
    // each bit stands alone and nothing is ordered by it
    if down {
      word.fetch_or(bit, Ordering::Relaxed);
    } else {
      word.fetch_and(!bit, Ordering::Relaxed);
    }
  }

  /// Puts a note down as a key of the panel plays it.
  ///
  /// The velocity is kept from 1 to 127: none at all would be the note coming up.
  ///
  /// # Panics
  ///
  /// Panics if `note` is over 127.
  pub(crate) fn press(&self, note: u8, velocity: u8) {
    let velocity = u32::from(velocity.clamp(1, 127));
    let pressed = |slot: u32| (slot & !VELOCITY).wrapping_add(PRESS) | DOWN | velocity;
    self.slots.played[usize::from(note)].update(Ordering::Relaxed, Ordering::Relaxed, pressed);
  }

  /// Lets a note of the panel up.
  ///
  /// # Panics
  ///
  /// Panics if `note` is over 127.
  pub(crate) fn release(&self, note: u8) {
    self.slots.played[usize::from(note)].fetch_and(!DOWN, Ordering::Relaxed);
  }

  /// Returns the notes the audio thread shows as down.
  pub(crate) fn shown(&self) -> Pressed {
    let [low, high] = &self.slots.shown;
    let (low, high) = (low.load(Ordering::Relaxed), high.load(Ordering::Relaxed));
    Pressed(u128::from(low) | u128::from(high) << 64)
  }
}

/// The notes as an audio thread last heard them.
///
/// They are those of one [`Notes`]. The audio thread keeps this and hands it to each
/// [`Notes::hear`].
#[derive(Clone, Debug)]
pub struct Heard([u32; NOTES]);

impl Heard {
  /// Creates the record of a thread that has heard no note.
  ///
  /// Every one of `notes` is up in it. The first [`Notes::hear`] tells the beginning of each
  /// note that is down by then, and nothing of the notes played before this was made.
  pub fn new(notes: &Notes) -> Self {
    let played = &notes.slots.played;
    Self(std::array::from_fn(|note| {
      played[note].load(Ordering::Relaxed) & !DOWN
    }))
  }
}

/// A note of the panel beginning or ending.
///
/// [`Notes::hear`] tells of these.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Played {
  /// The note went down.
  On {
    /// The note, as a MIDI note number from 0 to 127.
    note: u8,
    /// The velocity it was struck with, from 1 to 127.
    velocity: u8,
  },
  /// The note came up.
  Off {
    /// The note, as a MIDI note number from 0 to 127.
    note: u8,
  },
}

/// The notes whose keys are down, of the 128 there are.
///
/// They are the notes a host plays and the one the panel holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pressed(u128);

impl Pressed {
  /// Returns whether a note is down.
  ///
  /// No note over 127 is.
  pub fn holds(self, note: u8) -> bool {
    1_u128
      .checked_shl(u32::from(note))
      .is_some_and(|bit| self.0 & bit != 0)
  }

  /// Returns whether any note is down.
  pub(crate) fn any(self) -> bool {
    self.0 != 0
  }

  /// Returns the notes with one more of them down, if it is one.
  pub(crate) fn with(self, note: Option<u8>) -> Self {
    let bit = note.and_then(|note| 1_u128.checked_shl(u32::from(note)));
    Self(self.0 | bit.unwrap_or(0))
  }
}

#[cfg(test)]
mod tests {
  use super::Played::{Off, On};
  use super::*;

  /// Returns what a thread hears of the notes since it last did.
  fn told(notes: &Notes, heard: &mut Heard) -> Vec<Played> {
    let mut told = Vec::new();
    notes.hear(heard, |played| told.push(played));
    told
  }

  fn on(note: u8, velocity: u8) -> Played {
    On { note, velocity }
  }

  #[test]
  fn a_thread_hears_each_note_begin_and_end_once_however_far_apart_it_looks() {
    let notes = Notes::new();
    let mut heard = Heard::new(&notes);
    assert_eq!(told(&notes, &mut heard), []);
    notes.press(60, 100);
    notes.press(0, 1);
    notes.press(127, 127);
    // lowest first, and none of them again while they are held
    assert_eq!(
      told(&notes, &mut heard),
      [on(0, 1), on(60, 100), on(127, 127)]
    );
    assert_eq!(told(&notes, &mut heard), []);
    notes.release(60);
    assert_eq!(told(&notes, &mut heard), [Off { note: 60 }]);
    notes.release(60);
    assert_eq!(told(&notes, &mut heard), []);

    // let go and struck again between two looks: an end, then a beginning at the new velocity
    notes.release(0);
    notes.press(0, 90);
    assert_eq!(told(&notes, &mut heard), [Off { note: 0 }, on(0, 90)]);
    // and at the same velocity, which leaves the slot different all the same
    notes.release(0);
    notes.press(0, 90);
    assert_eq!(told(&notes, &mut heard), [Off { note: 0 }, on(0, 90)]);
    // struck and let go between two looks: a beginning, then an end
    notes.press(64, 50);
    notes.release(64);
    assert_eq!(told(&notes, &mut heard), [on(64, 50), Off { note: 64 }]);
    // let go, struck and let go: the held note ends, and the short one begins and ends
    notes.release(127);
    notes.press(127, 3);
    notes.release(127);
    assert_eq!(
      told(&notes, &mut heard),
      [Off { note: 127 }, on(127, 3), Off { note: 127 }]
    );
    // several presses between two looks are one
    for _ in 0..3 {
      notes.press(10, 20);
      notes.release(10);
    }
    notes.press(10, 30);
    assert_eq!(told(&notes, &mut heard), [on(10, 30)]);
  }

  #[test]
  fn every_end_is_told_before_any_beginning_whichever_way_a_slide_went() {
    let notes = Notes::new();
    let mut heard = Heard::new(&notes);
    notes.press(10, 30);
    told(&notes, &mut heard);
    // up the keys, and back down them
    notes.release(10);
    notes.press(11, 30);
    assert_eq!(told(&notes, &mut heard), [Off { note: 10 }, on(11, 30)]);
    notes.release(11);
    notes.press(10, 40);
    assert_eq!(told(&notes, &mut heard), [Off { note: 11 }, on(10, 40)]);
    // down over a key that was only passed: its end still follows its own beginning
    notes.release(10);
    notes.press(9, 40);
    notes.release(9);
    notes.press(8, 40);
    assert_eq!(
      told(&notes, &mut heard),
      [Off { note: 10 }, on(8, 40), on(9, 40), Off { note: 9 }]
    );
  }

  #[test]
  fn a_thread_that_begins_to_listen_hears_what_is_down_and_nothing_older() {
    let notes = Notes::new();
    notes.press(40, 80);
    notes.release(40);
    notes.press(41, 80);
    // a clone is the same notes
    let mut heard = Heard::new(&notes.clone());
    assert_eq!(told(&notes, &mut heard), [on(41, 80)]);
    notes.release(41);
    assert_eq!(told(&notes, &mut heard), [Off { note: 41 }]);
  }

  #[test]
  fn a_press_is_never_struck_with_nothing_or_with_more_than_there_is() {
    let notes = Notes::new();
    let mut heard = Heard::new(&notes);
    notes.press(1, 0);
    notes.press(2, 255);
    assert_eq!(told(&notes, &mut heard), [on(1, 1), on(2, 127)]);
  }

  #[test]
  fn the_notes_shown_are_the_hosts_and_the_one_the_panel_holds() {
    let notes = Notes::new();
    assert!(!notes.shown().any());
    for note in [0, 63, 64, 127] {
      notes.show(note, true);
    }
    notes.show(63, false);
    // a note past the last is none of them
    notes.show(128, true);
    notes.show(255, true);
    let down = notes.shown().with(Some(72)).with(None);
    for note in 0..=255 {
      assert_eq!(down.holds(note), [0, 64, 72, 127].contains(&note), "{note}");
    }
    assert!(notes.shown().any());
    // a note of the panel is not one the audio thread shows
    let own = Notes::new();
    own.press(30, 100);
    assert_eq!(own.shown(), Pressed::default());
  }
}
