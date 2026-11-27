use crate::interaction::Action::{Begin, End, Redraw, Set};
use crate::interaction::bench::{Bench, FORMANT, MIX, PITCH, SCALES, SIZE, STEPPED};
use crate::interaction::*;

/// Creates the bench on two pages, with the first shown.
///
/// The shifts are on the first page, and the mix and the stepped shift on the second.
fn two_pages() -> Bench {
  let mut bench = Bench::new();
  let mut controls = bench.interaction.controls.clone();
  for control in &mut controls[MIX..] {
    control.page = 1;
  }
  let dials = std::mem::take(&mut bench.interaction.dials);
  bench.interaction = Interaction::paged(dials, controls, SIZE, SCALES, 2, 0);
  bench
}

/// The tab of a page.
fn tab(page: usize) -> Option<Target> {
  Some(Target::Page(page))
}

#[test]
fn a_control_on_a_page_not_shown_takes_no_input() {
  let mut bench = two_pages();
  assert_eq!(bench.interaction.page(), 0);
  // a press, a double click on its readout, the wheel and the right button all go nowhere
  for over in [Target::Control(MIX), Target::Readout(MIX)] {
    for _ in 0..2 {
      assert_eq!(bench.press(Some(over), 100.0), []);
      assert_eq!(bench.drag(40.0, false), []);
      assert_eq!(bench.release(), []);
    }
    assert_eq!(bench.wheel(over, 3.0, false), []);
    assert_eq!(bench.right(Some(over)), []);
  }
  assert_eq!(bench.reset(Some(Target::Control(STEPPED))), []);
  let interaction = &bench.interaction;
  assert!(interaction.edit().is_none() && interaction.menu().is_none());
  assert_eq!(interaction.held(), None);
  // its tab shows it, and then it answers and the first page's controls do not
  assert_eq!(bench.press(tab(1), 100.0), [Redraw]);
  bench.release();
  assert_eq!(bench.interaction.page(), 1);
  assert_eq!(bench.wheel(Target::Control(PITCH), 1.0, false), []);
  assert_eq!(bench.press(Some(Target::Control(FORMANT)), 100.0), []);
  assert_eq!(bench.drag(40.0, false), []);
  bench.release();
  let wheeled = bench.wheel(Target::Control(MIX), -1.0, false);
  assert_eq!(wheeled[0], Begin(MIX));
}

#[test]
fn a_press_on_the_tab_shown_does_nothing() {
  let mut bench = two_pages();
  assert_eq!(bench.press(tab(0), 100.0), []);
  bench.release();
  // nor does a second press in time, which is no double click to a tab
  assert_eq!(bench.press(tab(1), 100.0), [Redraw]);
  bench.release();
  assert_eq!(bench.press(tab(1), 100.0), []);
  assert_eq!(bench.interaction.page(), 1);
}

#[test]
fn a_switch_sets_no_value_and_ends_what_was_under_way() {
  let mut bench = two_pages();
  let before = bench.interaction.readings(&bench.values);
  // a drag whose release never came ends, and the switch sets nothing
  bench.press(Some(Target::Control(PITCH)), 100.0);
  assert_eq!(bench.drag(80.0, false)[0], Begin(PITCH));
  assert_eq!(bench.press(tab(1), 100.0), [End(PITCH), Redraw]);
  bench.release();
  assert_eq!(bench.drag(20.0, false), []);
  // and back, with every reading as it was for values that stand where they did
  assert_eq!(bench.press(tab(0), 100.0), [Redraw]);
  bench.release();
  assert_eq!(bench.interaction.readings(&bench.values), before);
  // a value being typed is set as a press anywhere else sets it
  bench.edit(FORMANT);
  bench.kind("12");
  let typed = bench.press(tab(1), 100.0);
  assert_eq!(&typed[..2], [Begin(FORMANT), Set(FORMANT, 0.75)]);
  assert_eq!(&typed[2..], [End(FORMANT), Redraw, Redraw]);
  assert!(bench.interaction.edit().is_none());
}

#[test]
#[should_panic(expected = "the tab is of one of the panel's pages")]
fn a_tab_past_the_last_page_is_refused() {
  two_pages().press(tab(2), 100.0);
}

#[test]
#[should_panic(expected = "every control and the page shown are on the panel's pages")]
fn a_control_past_the_last_page_is_refused() {
  let mut bench = Bench::new();
  let mut controls = bench.interaction.controls.clone();
  controls[PITCH].page = 1;
  let dials = std::mem::take(&mut bench.interaction.dials);
  Interaction::new(dials, controls, SIZE, SCALES);
}
