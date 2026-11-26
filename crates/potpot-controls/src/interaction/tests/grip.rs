use std::time::Duration;

use crate::interaction::bench::{Bench, SCALES, SIZE, close};
use crate::interaction::*;

/// A point of the grip, in the far corner of the bench's panel.
///
/// It is ten points in from both edges.
const GRIP: Point = [SIZE[0] - 10.0, SIZE[1] - 10.0];

/// Moves the pointer to a place in the window.
fn pull(bench: &mut Bench, to: Point) -> Vec<Action> {
  // the same place in the window is fewer of the panel's points the larger it has become
  let at = to.map(|across| across / bench.scale);
  bench.send(Event::Move {
    at,
    window: to,
    over: None,
    fine: false,
  })
}

fn grab(bench: &mut Bench) -> Vec<Action> {
  let (at, window) = bench.place(GRIP);
  bench.send(Event::Press {
    button: Button::Left,
    at,
    window,
    over: Some(Target::Grip),
    reset: false,
  })
}

#[test]
fn pulling_the_grip_scales_the_window_to_reach_the_pointer() {
  let mut bench = Bench::new();
  assert_eq!(grab(&mut bench), []);
  // held where it was, the window stays as it is
  assert_eq!(pull(&mut bench, GRIP), []);
  // across by a tenth of the panel's width
  assert_eq!(
    pull(&mut bench, [GRIP[0] + 50.0, GRIP[1]]),
    [Action::Scale(1.1)]
  );
  // the window follows, and the pointer where it was asks for nothing new
  bench.scale = 1.1;
  assert_eq!(pull(&mut bench, [GRIP[0] + 50.0, GRIP[1]]), []);
  // down by more of the panel's height than that: whichever way the pointer has gone
  // further counts, and the panel keeps its shape
  assert_eq!(
    pull(&mut bench, [GRIP[0] + 50.0, GRIP[1] + 50.0]),
    [Action::Scale(1.2)]
  );
  bench.scale = 1.2;
  // back in across by more than it is still out down, the window shrinks to there
  let scaled = |actions: Vec<Action>| match actions[..] {
    [Action::Scale(scale)] => scale,
    _ => panic!("the actions are one scale: {actions:?}"),
  };
  assert!(close(
    scaled(pull(&mut bench, [GRIP[0] - 100.0, GRIP[1] + 25.0])),
    0.8
  ));
  // and straight along one side it follows that side alone
  bench.scale = 0.8;
  assert!(close(
    scaled(pull(&mut bench, [GRIP[0] + 250.0, GRIP[1]])),
    1.5
  ));
  assert_eq!(bench.release(), []);
  // and with the button up the pointer scales nothing
  assert_eq!(pull(&mut bench, [GRIP[0] + 200.0, GRIP[1]]), []);
}

#[test]
fn a_window_that_has_followed_the_pointer_asks_for_no_other_size() {
  let mut bench = Bench::new();
  grab(&mut bench);
  let to = [GRIP[0] + 51.0, GRIP[1]];
  assert_eq!(pull(&mut bench, to), [Action::Scale(1.102)]);
  // the panel fitted into the window's whole pixels is a hair wider or narrower than that, so
  // the host moving the pointer to where it already is puts it a little way off on the panel
  for (scale, off) in [(1.1019999, 0.4), (1.1020001, -0.4), (1.102, 0.0)] {
    bench.scale = scale;
    let moved = Event::Move {
      at: [to[0] / scale + off, to[1] / scale],
      window: to,
      over: None,
      fine: false,
    };
    assert_eq!(bench.send(moved), [], "at {scale}");
  }
}

#[test]
fn the_grip_held_anywhere_on_it_does_not_jump_the_window() {
  // at a window half as large again, held three points off `GRIP` each way
  let mut bench = Bench::new();
  bench.scale = 1.5;
  let (held, on_screen) = bench.place([GRIP[0] + 3.0, GRIP[1] - 3.0]);
  bench.send(Event::Press {
    button: Button::Left,
    at: held,
    window: on_screen,
    over: Some(Target::Grip),
    reset: false,
  });
  assert_eq!(pull(&mut bench, on_screen), []);
  assert_eq!(
    pull(&mut bench, [on_screen[0] + 25.0, on_screen[1]]),
    [Action::Scale(1.55)]
  );
}

#[test]
fn the_window_is_scaled_no_smaller_or_larger_than_its_limits() {
  let mut bench = Bench::new();
  grab(&mut bench);
  assert_eq!(
    pull(&mut bench, [5000.0, 5000.0]),
    [Action::Scale(SCALES[1])]
  );
  assert_eq!(pull(&mut bench, [6000.0, 5000.0]), []);
  assert_eq!(
    pull(&mut bench, [-200.0, -200.0]),
    [Action::Scale(SCALES[0])]
  );
  assert_eq!(pull(&mut bench, [-300.0, -200.0]), []);
}

#[test]
fn a_double_click_on_the_grip_is_the_windows_own_size() {
  let mut bench = Bench::new();
  bench.scale = 2.0;
  grab(&mut bench);
  bench.release();
  assert_eq!(grab(&mut bench), [Action::Scale(1.0)]);
  // which is all of it: the pointer moving on with the button down pulls nothing
  assert_eq!(pull(&mut bench, [900.0, 400.0]), []);
  // and the other button on the grip does nothing
  bench.after(Duration::from_secs(1), Event::Tick);
  assert_eq!(bench.right(Some(Target::Grip)), []);
  assert!(bench.interaction.menu().is_none());
}

#[test]
fn a_press_while_the_grip_is_pulled_lets_the_grip_go_first() {
  let mut bench = Bench::new();
  grab(&mut bench);
  assert_eq!(bench.press(None, 100.0), []);
  // with no button down on the grip the pointer scales nothing
  assert_eq!(pull(&mut bench, [GRIP[0] + 50.0, GRIP[1]]), []);
}
