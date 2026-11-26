//! Numbers an audio thread hands the skin of an open window.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

/// A row of numbers an audio thread writes for a skin.
///
/// It has a fixed count of slots and carries what no parameter holds: a meter's level, a gain
/// reduction, the bins of a spectrum, the trace of a scope. The opener makes one, puts a clone in
/// [`Settings::feed`](crate::Settings::feed) and keeps another to write with. Every clone
/// writes and reads the same slots.
///
/// The window hands the slots to the skin as [`View::feed`](crate::View::feed) and draws a
/// frame when they differ from the ones last drawn. Nothing tells the window of a write. It
/// looks at the slots at each of its ticks, and once a frame has drawn a change, at each of
/// its frames until a tick has passed with no further change.
///
/// Each slot is written and read on its own. A frame can show some slots from before a run of
/// [`Feed::set`] calls and the others from after it.
#[derive(Clone, Debug, Default)]
pub struct Feed {
  /// The bits of each slot's `f32`.
  slots: Arc<[AtomicU32]>,
}

impl Feed {
  /// Creates a feed of `slots` numbers, each zero.
  pub fn new(slots: usize) -> Self {
    Self {
      slots: (0..slots)
        .map(|_| AtomicU32::new(0.0_f32.to_bits()))
        .collect(),
    }
  }

  /// Sets the number in a slot.
  ///
  /// This is one atomic store. It neither allocates, locks nor calls into the system, so an
  /// audio thread can call it.
  ///
  /// # Panics
  ///
  /// Panics if `slot` is past the last of the feed's slots.
  pub fn set(&self, slot: usize, value: f32) {
    // each slot stands alone and nothing is ordered by it
    self.slots[slot].store(value.to_bits(), Ordering::Relaxed);
  }

  /// Returns the number in each slot as it is now.
  pub(crate) fn read(&self) -> Vec<f32> {
    (self.slots.iter())
      .map(|slot| f32::from_bits(slot.load(Ordering::Relaxed)))
      .collect()
  }
}

/// Returns whether two readings of a feed are the same.
///
/// The numbers are compared by their bits, so that a slot holding a NaN counts as still.
pub(crate) fn same(one: &[f32], other: &[f32]) -> bool {
  let bits = |number: &f32| number.to_bits();
  one.iter().map(bits).eq(other.iter().map(bits))
}
