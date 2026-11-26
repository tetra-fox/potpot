use std::time::Duration;

use crate::interaction::Action::{Begin, End, Set};
use crate::interaction::bench::{
  Bench, FORMANT, MIX, PITCH, STEPPED, close, pad, semitones, set, slide,
};
use crate::interaction::*;

#[test]
fn the_wheel_takes_a_stepped_control_a_step_and_onto_one() {
  let mut bench = Bench::new();
  let knob = Target::Control(STEPPED);
  assert_eq!(
    bench.wheel(knob, 1.0, false)[1..],
    [Set(STEPPED, semitones(1.0))]
  );
  assert_eq!(
    bench.wheel(knob, -1.0, false),
    [Set(STEPPED, semitones(0.0))]
  );
  // with the fine key a tenth of a step, off the steps
  assert!(close(
    set(&bench.wheel(knob, 1.0, true), STEPPED),
    semitones(0.1)
  ));
  // and from between two steps a line goes to the next one that way
  assert_eq!(
    bench.wheel(knob, 1.0, false),
    [Set(STEPPED, semitones(1.0))]
  );
  bench.wheel(knob, -1.0, true);
  assert_eq!(
    bench.wheel(knob, -1.0, false),
    [Set(STEPPED, semitones(0.0))]
  );
  // it stops at the end of the turn
  bench.after(Duration::from_secs(1), Event::Tick);
  bench.values[STEPPED] = semitones(23.5);
  assert_eq!(bench.wheel(knob, 1.0, false)[1..], [Set(STEPPED, 1.0)]);
  assert_eq!(bench.wheel(knob, 1.0, false), [Set(STEPPED, 1.0)]);
}

#[test]
fn a_turn_of_no_lines_tells_the_host_nothing() {
  // what a wheel turned sideways comes in as
  let mut bench = Bench::new();
  assert_eq!(bench.wheel(Target::Control(STEPPED), 0.0, false), []);
  assert_eq!(bench.wheel(Target::Control(PITCH), 0.0, false), []);
  assert_eq!(bench.wheel(Target::Control(PITCH), -0.0, true), []);
  // and no gesture is left open by it
  assert_eq!(bench.send(Event::Closed), []);
}

#[test]
fn parts_of_a_line_gather_into_the_steps_of_a_stepped_control() {
  let mut bench = Bench::new();
  let knob = Target::Control(STEPPED);
  // a twentieth of a line at a time, as a trackpad sends them
  for _ in 0..19 {
    assert_eq!(bench.wheel(knob, 0.05, false), []);
  }
  assert_eq!(
    bench.wheel(knob, 0.05, false),
    [Begin(STEPPED), Set(STEPPED, semitones(1.0))]
  );
  // a line and a half is one step now, and the half makes the next with another
  assert_eq!(
    bench.wheel(knob, -1.5, false),
    [Set(STEPPED, semitones(0.0))]
  );
  assert_eq!(
    bench.wheel(knob, -0.5, false),
    [Set(STEPPED, semitones(-1.0))]
  );
  assert_eq!(
    bench.after(Duration::from_secs(1), Event::Tick),
    [End(STEPPED)]
  );
}

#[test]
fn less_than_a_line_on_a_stepped_control_tells_the_host_nothing() {
  let mut bench = Bench::new();
  let knob = Target::Control(STEPPED);
  for _ in 0..3 {
    assert_eq!(bench.wheel(knob, 0.3, false), []);
  }
  // the wheel rests with no gesture to end, and what had gathered goes
  assert_eq!(bench.after(Duration::from_secs(1), Event::Tick), []);
  assert_eq!(bench.wheel(knob, 0.3, false), []);
  // as it does when the wheel moves to another control and back
  let other = bench.wheel(Target::Control(PITCH), 1.0, false);
  assert_eq!(other[0], Begin(PITCH));
  assert_eq!(bench.wheel(knob, 0.9, false), [End(PITCH)]);
  assert_eq!(bench.press(None, 100.0), []);
}

#[test]
fn a_burst_of_the_wheel_is_one_gesture() {
  let mut bench = Bench::new();
  let knob = Target::Control(STEPPED);
  // a semitone of the 48 the stepped shift covers for each line
  let first = bench.wheel(knob, 1.0, false);
  assert_eq!(first[0], Begin(STEPPED));
  assert!(close(set(&first[1..], STEPPED), 0.5 + 1.0 / 48.0));
  assert!(close(
    set(&bench.wheel(knob, 1.0, false), STEPPED),
    0.5 + 2.0 / 48.0
  ));
  assert!(close(
    set(&bench.wheel(knob, -1.0, true), STEPPED),
    0.5 + 1.9 / 48.0
  ));
  // it stays open while the wheel has only just stopped
  assert_eq!(bench.after(Duration::from_millis(300), Event::Tick), []);
  assert_eq!(
    bench.after(Duration::from_millis(200), Event::Tick),
    [End(STEPPED)]
  );
  assert_eq!(bench.send(Event::Tick), []);
  // the next turn is a new one
  assert_eq!(bench.wheel(knob, 1.0, false)[0], Begin(STEPPED));
}

#[test]
fn the_wheel_moving_on_or_a_press_ends_its_gesture() {
  let mut bench = Bench::new();
  bench.wheel(Target::Control(PITCH), 1.0, false);
  let next = bench.wheel(Target::Readout(MIX), -1.0, false);
  assert_eq!(next[..2], [End(PITCH), Begin(MIX)]);
  assert!(close(set(&next[2..], MIX), 0.95));
  assert_eq!(bench.press(None, 100.0), [End(MIX)]);
  // over nothing that turns it does nothing
  assert_eq!(
    bench.send(Event::Wheel {
      over: None,
      lines: 1.0,
      fine: false
    }),
    []
  );
}

#[test]
fn the_wheel_turns_the_parameter_a_control_gives_it_or_else_its_own() {
  let mut bench = pad(FORMANT);
  bench.interaction.controls[PITCH].wheel = Some(STEPPED);
  let puck = Target::Control(PITCH);
  assert_eq!(
    bench.wheel(puck, 1.0, false),
    [Begin(STEPPED), Set(STEPPED, semitones(1.0))]
  );
  // a press ends the wheel's gesture, and the drag after it is on the pad's other two
  assert_eq!(bench.press(Some(puck), 100.0), [End(STEPPED)]);
  assert_eq!(
    slide(&mut bench, [132.0, 84.0]),
    [
      Begin(PITCH),
      Set(PITCH, 0.75),
      Begin(FORMANT),
      Set(FORMANT, 0.75)
    ]
  );
  // given none, the wheel turns the pad's own parameter, which is the one up
  let mut bench = pad(FORMANT);
  let first = bench.wheel(puck, 1.0, false);
  assert_eq!(first[0], Begin(PITCH));
  assert!(close(set(&first[1..], PITCH), 0.55));
  assert_eq!(
    bench.after(Duration::from_secs(1), Event::Tick),
    [End(PITCH)]
  );
}
