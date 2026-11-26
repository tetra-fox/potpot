use crate::interaction::Action::{Begin, End, Set};
use crate::interaction::bench::{
  Bench, FORMANT, MIX, PITCH, STEPPED, close, pad, semitones, set, slide,
};
use crate::interaction::*;

#[test]
fn a_drag_is_one_gesture_from_the_first_move_to_the_release() {
  let mut bench = Bench::new();
  assert_eq!(bench.press(Some(Target::Control(PITCH)), 100.0), []);
  // held from the press on, before it has moved
  assert_eq!(bench.interaction.held(), Some(PITCH));
  let first = bench.drag(80.0, false);
  assert_eq!(first[0], Begin(PITCH));
  assert!(close(set(&first[1..], PITCH), 0.6));
  assert!(close(set(&bench.drag(90.0, false), PITCH), 0.55));
  assert_eq!(bench.release(), [End(PITCH)]);
  assert_eq!(bench.interaction.held(), None);
  // and with the button up the pointer turns nothing
  assert_eq!(bench.drag(50.0, false), []);
}

#[test]
fn a_click_that_does_not_move_tells_the_host_nothing() {
  let mut bench = Bench::new();
  assert_eq!(bench.press(Some(Target::Control(PITCH)), 100.0), []);
  assert_eq!(bench.drag(100.0, false), []);
  assert_eq!(bench.release(), []);
}

#[test]
fn the_fine_key_changes_the_pace_of_a_drag_and_not_its_place() {
  let mut bench = Bench::new();
  bench.press(Some(Target::Control(PITCH)), 100.0);
  assert!(close(set(&bench.drag(80.0, false)[1..], PITCH), 0.6));
  // held from here: the same way again moves a tenth as far, from where the knob is
  assert!(close(set(&bench.drag(60.0, true), PITCH), 0.61));
  // and let go: the knob goes on from there at its full pace
  assert!(close(set(&bench.drag(40.0, false), PITCH), 0.71));
  assert!(close(set(&bench.drag(60.0, true), PITCH), 0.70));
}

#[test]
fn a_drag_stops_at_the_end_of_the_turn_and_comes_back_at_once() {
  let mut bench = Bench::new();
  bench.press(Some(Target::Control(PITCH)), 300.0);
  assert_eq!(set(&bench.drag(0.0, false)[1..], PITCH), 1.0);
  // further the same way there is nothing new to tell the host
  assert_eq!(bench.drag(-50.0, false), []);
  assert!(close(set(&bench.drag(-30.0, false), PITCH), 0.9));
}

#[test]
fn a_stepped_control_is_dragged_from_step_to_step_and_says_so_once_each() {
  // five points of dragging to a semitone: the whole 48 take 240
  let mut bench = Bench::new();
  bench.press(Some(Target::Control(STEPPED)), 100.0);
  // not yet half way to the next step
  assert_eq!(bench.drag(98.0, false), []);
  let first = bench.drag(97.0, false);
  assert_eq!(first, [Begin(STEPPED), Set(STEPPED, semitones(1.0))]);
  // on toward the step after, and past the half way to it
  assert_eq!(bench.drag(96.0, false), []);
  assert_eq!(bench.drag(93.0, false), []);
  assert_eq!(bench.drag(92.0, false), [Set(STEPPED, semitones(2.0))]);
  // a point at a time the pointer still gets there: nothing is lost between the steps
  let mut sets = Vec::new();
  for y in (72..92).rev() {
    sets.extend(bench.drag(y as f32, false));
  }
  assert_eq!(
    sets,
    [3.0, 4.0, 5.0, 6.0].map(|plain| Set(STEPPED, semitones(plain)))
  );
  assert_eq!(bench.release(), [End(STEPPED)]);
}

#[test]
fn the_fine_key_turns_a_stepped_control_smoothly_from_where_it_shows() {
  let mut bench = Bench::new();
  bench.press(Some(Target::Control(STEPPED)), 100.0);
  // three points on, the knob shows a semitone though the pointer has taken it 0.6
  bench.drag(97.0, false);
  // the key going down moves nothing
  assert_eq!(bench.drag(97.0, true), []);
  // and from there ten points are a fifth of a semitone, on from the one shown
  assert!(close(set(&bench.drag(87.0, true), STEPPED), semitones(1.2)));
  assert!(close(set(&bench.drag(77.0, true), STEPPED), semitones(1.4)));
  // the key coming up puts the knob on the nearest step at once
  assert_eq!(bench.drag(77.0, false), [Set(STEPPED, semitones(1.0))]);
  // and it steps on from where the pointer had it, 1.4: one more point makes 1.6
  assert_eq!(bench.drag(76.0, false), [Set(STEPPED, semitones(2.0))]);
}

#[test]
fn a_stepped_control_off_its_steps_goes_to_one_with_the_first_move() {
  for (y, lands) in [(99.0, 8.0), (101.0, 7.0)] {
    let mut bench = Bench::new();
    bench.values[STEPPED] = semitones(7.39);
    bench.press(Some(Target::Control(STEPPED)), 100.0);
    // a point either way is a fifth of a semitone, and what shows is the step nearest that
    let moved = bench.drag(y, false);
    assert_eq!(
      moved,
      [Begin(STEPPED), Set(STEPPED, semitones(lands))],
      "{y}"
    );
  }
  // with the fine key it turns on from where it is
  let mut bench = Bench::new();
  bench.values[STEPPED] = semitones(7.39);
  bench.press(Some(Target::Control(STEPPED)), 100.0);
  assert!(close(
    set(&bench.drag(90.0, true)[1..], STEPPED),
    semitones(7.59)
  ));
}

/// Creates the bench with its mix made a fader.
///
/// The fader slides to the right, a hundred points for its whole turn, and stands in the
/// middle.
fn fader() -> Bench {
  let mut bench = Bench::new();
  bench.interaction.controls[MIX].feel.drag = Drag::Along {
    way: [1.0, 0.0],
    turn: 0.01,
  };
  bench.values[MIX] = 0.5;
  bench
}

#[test]
fn a_fader_is_dragged_along_its_own_way_and_no_other() {
  let mut bench = fader();
  bench.press(Some(Target::Control(MIX)), 100.0);
  // up the panel is across a fader that lies on its side
  assert_eq!(slide(&mut bench, [100.0, 40.0]), []);
  let first = slide(&mut bench, [120.0, 40.0]);
  assert_eq!(first[0], Begin(MIX));
  assert!(close(set(&first[1..], MIX), 0.7));
  assert!(close(set(&slide(&mut bench, [90.0, 300.0]), MIX), 0.4));
  assert_eq!(bench.release(), [End(MIX)]);
}

#[test]
fn a_pad_is_one_gesture_on_each_parameter_from_its_first_change_to_the_release() {
  let mut bench = pad(FORMANT);
  assert_eq!(bench.press(Some(Target::Control(PITCH)), 100.0), []);
  // straight to the right the host hears of the formant alone
  assert_eq!(
    slide(&mut bench, [132.0, 100.0]),
    [Begin(FORMANT), Set(FORMANT, 0.75)]
  );
  // up and part of the way back at once: the pitch begins here, and the formant goes on
  assert_eq!(
    slide(&mut bench, [116.0, 84.0]),
    [Begin(PITCH), Set(PITCH, 0.75), Set(FORMANT, 0.625)]
  );
  assert_eq!(slide(&mut bench, [116.0, 92.0]), [Set(PITCH, 0.625)]);
  assert_eq!(bench.release(), [End(PITCH), End(FORMANT)]);
  assert_eq!(slide(&mut bench, [0.0, 0.0]), []);
}

#[test]
fn a_pad_dragged_one_way_ends_no_gesture_on_its_other_parameter() {
  let mut bench = pad(FORMANT);
  bench.press(Some(Target::Control(PITCH)), 100.0);
  assert_eq!(
    slide(&mut bench, [100.0, 84.0]),
    [Begin(PITCH), Set(PITCH, 0.75)]
  );
  // the window goes with the button still down
  assert_eq!(bench.send(Event::Closed), [End(PITCH)]);
}

#[test]
fn a_pad_lands_on_the_steps_of_one_parameter_at_the_pace_of_the_other() {
  // the stepped shift across: its 48 semitones over the 128 points, with no slowing for them
  let mut bench = pad(STEPPED);
  bench.press(Some(Target::Control(PITCH)), 100.0);
  // two points are three quarters of a semitone, which shows as one
  assert_eq!(
    slide(&mut bench, [102.0, 96.0]),
    [
      Begin(PITCH),
      Set(PITCH, 0.5625),
      Begin(STEPPED),
      Set(STEPPED, semitones(1.0))
    ]
  );
  // a point on is short of half way to the next, and only the pitch has anything new
  assert_eq!(slide(&mut bench, [103.0, 92.0]), [Set(PITCH, 0.625)]);
  let fine = |bench: &mut Bench, at: Point| {
    bench.send(Event::Move {
      at,
      window: at,
      over: None,
      fine: true,
    })
  };
  // the fine key going down moves neither, though the pointer has the stepped one past the
  // semitone it shows
  assert_eq!(fine(&mut bench, [103.0, 92.0]), []);
  // and from where each shows both go a tenth as fast, the stepped one off its steps
  match fine(&mut bench, [113.0, 60.0])[..] {
    [Set(PITCH, pitch), Set(STEPPED, stepped)] => {
      assert!(close(pitch, 0.675), "{pitch}");
      assert!(close(stepped, semitones(1.375)), "{stepped}");
    }
    ref other => panic!("the actions set both values: {other:?}"),
  }
}

#[test]
fn a_pad_past_an_end_of_its_room_waits_for_the_pointer_to_come_back() {
  let mut bench = pad(STEPPED);
  bench.press(Some(Target::Control(PITCH)), 100.0);
  // the end of the room is 64 points to the right, and the pointer goes 200
  assert_eq!(
    slide(&mut bench, [300.0, 100.0]),
    [Begin(STEPPED), Set(STEPPED, 1.0)]
  );
  // on its way back the puck stays at the end until the pointer is over the room again
  assert_eq!(slide(&mut bench, [290.0, 100.0]), []);
  assert_eq!(slide(&mut bench, [164.0, 100.0]), []);
  // a quarter of the room back in is a quarter of the turn, a semitone for a semitone
  assert_eq!(
    slide(&mut bench, [132.0, 100.0]),
    [Set(STEPPED, semitones(12.0))]
  );
}

#[test]
fn a_drag_goes_on_from_where_the_host_has_taken_a_parameter_it_has_yet_to_change() {
  // a knob held still while the host moves it, then dragged a point
  let mut bench = Bench::new();
  bench.press(Some(Target::Control(PITCH)), 100.0);
  bench.values[PITCH] = 0.875;
  assert_eq!(bench.drag(100.0, false), []);
  let moved = bench.drag(99.0, false);
  assert_eq!(moved[0], Begin(PITCH));
  assert!(close(set(&moved[1..], PITCH), 0.88));
  // a pad dragged across alone while the host moves the pitch, then a little way up
  let mut bench = pad(FORMANT);
  bench.press(Some(Target::Control(PITCH)), 100.0);
  slide(&mut bench, [132.0, 100.0]);
  bench.values[PITCH] = 0.25;
  assert_eq!(
    slide(&mut bench, [132.0, 92.0]),
    [Begin(PITCH), Set(PITCH, 0.375)]
  );
}

#[test]
fn a_pad_dragged_one_way_begins_nothing_on_a_stepped_parameter_the_other_way() {
  // the stepped shift across, standing a hair off a step, as a host hands one back
  let mut bench = pad(STEPPED);
  bench.values[STEPPED] = 0.5 + 1e-6;
  bench.press(Some(Target::Control(PITCH)), 100.0);
  assert_eq!(
    slide(&mut bench, [100.0, 92.0]),
    [Begin(PITCH), Set(PITCH, 0.625)]
  );
  // the host moves it between two steps, and the drag goes on straight up
  bench.values[STEPPED] = 0.613;
  assert_eq!(slide(&mut bench, [100.0, 84.0]), [Set(PITCH, 0.75)]);
  assert_eq!(bench.release(), [End(PITCH)]);
}

#[test]
fn travel_past_an_end_is_dropped_when_the_host_moves_a_parameter_not_yet_begun() {
  // the formant across is at the end of its turn, and the pointer goes 50 points past it
  let mut bench = pad(FORMANT);
  bench.values[FORMANT] = 1.0;
  bench.press(Some(Target::Control(PITCH)), 100.0);
  assert_eq!(slide(&mut bench, [150.0, 100.0]), []);
  // the host takes it to the middle: from there it goes as far as the pointer does
  bench.values[FORMANT] = 0.5;
  assert_eq!(
    slide(&mut bench, [134.0, 100.0]),
    [Begin(FORMANT), Set(FORMANT, 0.375)]
  );
}
