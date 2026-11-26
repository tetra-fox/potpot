//! The places of parts along a panel's width or height.

/// The distance in points sizes may run past their span.
///
/// Sizes that add up to the width of a span an earlier call returned can come out a little
/// over it from rounding.
const FIT_SLACK: f32 = 1e-3;

/// Where the room a span has spare goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pack {
  /// The spare room after the last slot.
  ///
  /// The slots touch, the first at the span's start.
  Start,
  /// The spare room split equally between the two ends.
  ///
  /// The slots touch in the middle of the span.
  Center,
  /// The spare room split equally between the slots.
  ///
  /// The first slot starts at the span's start and the last ends at its end. A single slot is
  /// placed as with [`Pack::Start`].
  Spread,
}

/// Returns the places of slots of some sizes in a span.
///
/// `span` is a start and an end, and `sizes` are lengths along it, all in points. The result
/// holds one `[start, end]` for each of `sizes`, in the same order, placed as `how` says.
///
/// # Panics
///
/// Panics if any of `sizes` is negative or NaN.
///
/// Panics if either end of `span` is NaN, or `sizes` add up to more than a thousandth of a point
/// over its length.
pub fn pack(span: [f32; 2], sizes: &[f32], how: Pack) -> Vec<[f32; 2]> {
  assert!(sizes.iter().all(|&size| size >= 0.0), "no size is negative");
  let [start, end] = span;
  let length = end - start;
  let taken: f32 = sizes.iter().sum();
  assert!(taken <= length + FIT_SLACK, "the sizes fit the span");
  let spare = length - taken;
  let (first, gap) = match (how, sizes.len()) {
    (Pack::Start, _) | (Pack::Spread, 0 | 1) => (start, 0.0),
    (Pack::Center, _) => (start + spare / 2.0, 0.0),
    (Pack::Spread, count) => (start, spare / (count - 1) as f32),
  };
  let mut at = first;
  sizes
    .iter()
    .map(|&size| {
      let slot = [at, at + size];
      at += size + gap;
      slot
    })
    .collect()
}

#[cfg(test)]
mod tests {
  use super::*;

  const MODES: [Pack; 3] = [Pack::Start, Pack::Center, Pack::Spread];

  fn near(a: f32, b: f32) -> bool {
    (a - b).abs() <= FIT_SLACK
  }

  /// Returns spans and the sizes packed in each.
  ///
  /// The cases hold:
  ///
  /// - no sizes
  /// - zero sizes
  /// - one size
  /// - sizes that fill their span exactly
  /// - a span from an earlier call filled by sizes that add up to its width
  fn cases() -> Vec<([f32; 2], Vec<f32>)> {
    let spans = [[0.0, 100.0], [20.0, 500.0], [-30.0, 30.0]];
    let lists: [&[f32]; 6] = [
      &[],
      &[0.0],
      &[40.0],
      &[0.0, 0.0, 0.0],
      &[10.0, 0.0, 25.0],
      &[12.0, 20.0, 15.0, 7.0],
    ];
    let mut cases = Vec::new();
    for span in spans {
      for sizes in lists {
        cases.push((span, sizes.to_vec()));
      }
      let length = span[1] - span[0];
      cases.push((span, vec![length / 3.0; 3]));
      cases.push((span, vec![length / 4.0, length / 2.0, length / 4.0]));
    }
    // a row of groups spread over a panel, and the second group's span filled by thirds
    let groups = pack([24.0, 776.0], &[200.0, 68.0, 150.0], Pack::Spread);
    let [start, end] = groups[1];
    cases.push((groups[1], vec![(end - start) / 3.0; 3]));
    cases
  }

  #[test]
  fn every_slot_keeps_its_size_inside_the_span_without_overlapping_the_next() {
    for how in MODES {
      for (span, sizes) in cases() {
        let slots = pack(span, &sizes, how);
        assert_eq!(slots.len(), sizes.len(), "{how:?} {span:?} {sizes:?}");
        for (slot, size) in slots.iter().zip(&sizes) {
          assert!(near(slot[1] - slot[0], *size), "{how:?} {span:?} {sizes:?}");
          assert!(
            slot[0] >= span[0] - FIT_SLACK && slot[1] <= span[1] + FIT_SLACK,
            "{how:?} {span:?} {sizes:?}"
          );
        }
        for pair in slots.windows(2) {
          assert!(
            pair[0][1] <= pair[1][0] + FIT_SLACK,
            "{how:?} {span:?} {sizes:?}"
          );
        }
      }
    }
  }

  #[test]
  fn start_packs_the_slots_touching_from_the_spans_start() {
    for (span, sizes) in cases() {
      let slots = pack(span, &sizes, Pack::Start);
      if let Some(first) = slots.first() {
        assert!(near(first[0], span[0]), "{span:?} {sizes:?}");
      }
      for pair in slots.windows(2) {
        assert!(near(pair[0][1], pair[1][0]), "{span:?} {sizes:?}");
      }
    }
  }

  #[test]
  fn center_leaves_equal_room_at_each_end_and_the_slots_touch() {
    for (span, sizes) in cases() {
      let slots = pack(span, &sizes, Pack::Center);
      if let (Some(first), Some(last)) = (slots.first(), slots.last()) {
        assert!(
          near(first[0] - span[0], span[1] - last[1]),
          "{span:?} {sizes:?}"
        );
      }
      for pair in slots.windows(2) {
        assert!(near(pair[0][1], pair[1][0]), "{span:?} {sizes:?}");
      }
    }
  }

  #[test]
  fn spread_reaches_from_end_to_end_with_equal_gaps() {
    for (span, sizes) in cases() {
      let slots = pack(span, &sizes, Pack::Spread);
      match slots.as_slice() {
        [] => {}
        // one slot is packed from the start
        [only] => assert!(near(only[0], span[0]), "{span:?} {sizes:?}"),
        [first, .., last] => {
          assert!(near(first[0], span[0]), "{span:?} {sizes:?}");
          assert!(near(last[1], span[1]), "{span:?} {sizes:?}");
          let gap = slots[1][0] - first[1];
          for pair in slots.windows(2) {
            assert!(near(pair[1][0] - pair[0][1], gap), "{span:?} {sizes:?}");
          }
        }
      }
    }
  }

  #[test]
  #[should_panic(expected = "the sizes fit the span")]
  fn sizes_wider_than_the_span_do_not_fit() {
    pack([0.0, 100.0], &[60.0, 41.0], Pack::Start);
  }
}
