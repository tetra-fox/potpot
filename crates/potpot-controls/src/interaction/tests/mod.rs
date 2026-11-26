use std::time::{Duration, Instant};

use super::Action::{Begin, End, Focus, Redraw, Set, WantClipboard};
use super::bench::{
  Bench, FORMANT, MIX, PITCH, ROOM, SCALES, SIZE, choose, close, mix, pad, set, shift,
};
use super::*;

mod grip;
mod keybed;

#[test]
fn a_control_turns_the_parameter_it_names_and_not_the_one_at_its_own_place() {
  // two controls on four parameters: the first turns the mix, the second the pitch
  let dials = vec![shift(None), shift(None), mix(), shift(Some(1.0))];
  let readout = Readout {
    room: ROOM,
    shows: |_| true,
  };
  let on = |parameter| Control::new(parameter, readout);
  let mut panel = Interaction::new(dials, vec![on(MIX), on(PITCH)], SIZE, SCALES);
  let values = [0.5, 0.5, 1.0, 0.5];
  let moment = Moment {
    now: Instant::now(),
    values: &values,
    scale: 1.0,
  };
  // each control shows its own parameter, written as that parameter is
  let readings = panel.readings(&values);
  assert_eq!([readings[0].value, readings[1].value], [1.0, 0.5]);
  assert_eq!(
    (readings[0].text.as_str(), readings[0].unit.as_str()),
    ("100", "%")
  );
  assert_eq!(
    (readings[1].text.as_str(), readings[1].unit.as_str()),
    ("0.00", " st")
  );
  // a click with the reset key on the second control resets the pitch, which is the first
  // parameter
  let press = |over| Event::Press {
    button: Button::Left,
    at: [10.0, 10.0],
    window: [10.0, 10.0],
    over: Some(over),
    reset: true,
  };
  assert_eq!(
    panel.handle(press(Target::Control(1)), moment),
    [
      Action::Begin(PITCH),
      Action::Set(PITCH, 0.5),
      Action::End(PITCH)
    ]
  );
  assert_eq!(
    panel.handle(press(Target::Control(0)), moment),
    [Action::Begin(MIX), Action::Set(MIX, 1.0), Action::End(MIX)]
  );
  // and the wheel over the first control's readout turns the mix down
  let wheel = Event::Wheel {
    over: Some(Target::Readout(0)),
    lines: -1.0,
    fine: false,
  };
  let actions = panel.handle(wheel, moment);
  assert_eq!(actions[0], Action::Begin(MIX));
  assert!(matches!(actions[1], Action::Set(MIX, value) if value < 1.0));
}

#[test]
fn a_control_reads_what_is_typed_into_it_while_it_is_typed() {
  let mut bench = Bench::new();
  let values = bench.values;
  let said = |bench: &Bench| {
    let reading = &bench.interaction.readings(&values)[MIX];
    (reading.text.clone(), reading.unit.clone())
  };
  assert_eq!(said(&bench), ("100".to_owned(), "%".to_owned()));
  // typing begins with the value as it was written, and no unit after it
  let readout = Some(Target::Readout(MIX));
  bench.press(readout, 100.0);
  bench.press(readout, 100.0);
  assert_eq!(said(&bench), ("100".to_owned(), String::new()));
  bench.key(Key::Letter('4'));
  assert_eq!(said(&bench), ("4".to_owned(), String::new()));
  // the other controls go on reading their values
  let pitch = &bench.interaction.readings(&values)[PITCH];
  assert_eq!((pitch.text.as_str(), pitch.unit.as_str()), ("0.00", " st"));
  bench.key(Key::Escape);
  assert_eq!(said(&bench), ("100".to_owned(), "%".to_owned()));
}

#[test]
fn a_double_click_on_a_knob_resets_it() {
  let mut bench = Bench::new();
  let knob = Some(Target::Control(MIX));
  bench.press(knob, 100.0);
  bench.release();
  assert_eq!(
    bench.press(knob, 101.0),
    [Begin(MIX), Set(MIX, 1.0), End(MIX)]
  );
  // the reset is whole, so the press holds nothing, the release ends nothing and a drag does
  // not follow
  assert_eq!(bench.interaction.held(), None);
  assert_eq!(bench.drag(50.0, false), []);
  assert_eq!(bench.release(), []);
  // a third press in time is a first one again
  assert_eq!(bench.press(knob, 101.0), []);
  bench.release();
  assert_eq!(bench.press(knob, 101.0).len(), 3);
}

#[test]
fn presses_too_far_apart_are_not_a_double_click() {
  let mut bench = Bench::new();
  let knob = Some(Target::Control(MIX));
  bench.press(knob, 100.0);
  bench.release();
  assert_eq!(
    bench.after(Duration::from_millis(500), Event::Release(Button::Left)),
    []
  );
  assert_eq!(bench.press(knob, 100.0), []);
  bench.release();
  // near in time but not in place
  assert_eq!(bench.press(knob, 110.0), []);
  bench.release();
  // or on something else
  assert_eq!(bench.press(Some(Target::Control(PITCH)), 110.0), []);
}

#[test]
fn a_click_with_the_reset_key_resets() {
  let mut bench = Bench::new();
  let actions = bench.reset(Some(Target::Control(PITCH)));
  assert_eq!(actions, [Begin(PITCH), Set(PITCH, 0.5), End(PITCH)]);
}

#[test]
fn closing_ends_whatever_gesture_is_open() {
  let mut bench = Bench::new();
  bench.press(Some(Target::Control(PITCH)), 100.0);
  bench.drag(90.0, false);
  assert_eq!(bench.send(Event::Closed), [End(PITCH)]);
  bench.wheel(Target::Control(MIX), -1.0, false);
  assert_eq!(bench.send(Event::Closed), [End(MIX)]);
  assert_eq!(bench.send(Event::Closed), []);
}

#[test]
fn a_press_while_a_drag_is_open_ends_the_drag_first() {
  // a release that never came, as when the system takes the pointer away
  let mut bench = Bench::new();
  bench.press(Some(Target::Control(PITCH)), 100.0);
  assert_eq!(bench.drag(80.0, false)[0], Begin(PITCH));
  assert_eq!(bench.press(Some(Target::Control(MIX)), 100.0), [End(PITCH)]);
  // and the press is a drag of its own
  assert_eq!(bench.drag(120.0, false)[0], Begin(MIX));
  assert_eq!(bench.press(None, 200.0), [End(MIX)]);
  assert_eq!(bench.drag(150.0, false), []);
  assert_eq!(bench.release(), []);
}

#[test]
fn a_press_while_a_button_is_held_lets_the_button_go_first() {
  let mut bench = button(Push::Momentary);
  let over = Some(Target::Control(MIX));
  bench.press(over, 100.0);
  assert_eq!(
    bench.press(over, 100.0),
    [Set(MIX, 0.0), End(MIX), Redraw, Begin(MIX), Set(MIX, 1.0)]
  );
  assert_eq!(bench.press(None, 100.0), [Set(MIX, 0.0), End(MIX), Redraw]);
  assert_eq!(bench.interaction.held(), None);
  // a latching one was not let go: it comes back out as it was
  let mut bench = button(Push::Latching);
  bench.press(over, 100.0);
  assert_eq!(bench.press(None, 100.0), [Redraw]);
  assert_eq!(bench.interaction.held(), None);
  assert_eq!(bench.release(), []);
}

#[test]
fn a_drag_and_the_wheel_start_from_where_the_control_is_shown() {
  // a value set a moment ago may not have reached the parameter, which still has its default
  let mut bench = Bench::new();
  bench.values[PITCH] = 0.2;
  bench.press(Some(Target::Control(PITCH)), 100.0);
  let actions = bench.drag(80.0, false);
  assert!(close(set(&actions[1..], PITCH), 0.3));
  bench.release();
  let actions = bench.wheel(Target::Control(PITCH), 1.0, false);
  assert!(close(
    set(&actions[1..], PITCH),
    0.2 + Feel::default().wheel
  ));
}

#[test]
fn a_double_click_that_goes_on_moving_resets_and_does_not_drag() {
  let mut bench = Bench::new();
  let knob = Some(Target::Control(PITCH));
  bench.press(knob, 100.0);
  bench.drag(99.0, false);
  bench.release();
  // the second press, a point off the first, with the button held and the pointer moving on
  let reset = bench.press(knob, 99.0);
  assert_eq!(
    reset[reset.len() - 3..],
    [Begin(PITCH), Set(PITCH, 0.5), End(PITCH)]
  );
  assert_eq!(bench.drag(98.0, false), []);
  assert_eq!(bench.drag(60.0, false), []);
  assert_eq!(bench.release(), []);
  // nor does the wheel take the reset's gesture on: its own begins and ends
  assert_eq!(
    bench.wheel(Target::Control(PITCH), 1.0, false)[0],
    Begin(PITCH)
  );
}

#[test]
fn a_value_copied_from_the_menu_pastes_back_as_the_same_value() {
  let mut bench = Bench::new();
  // the pitch standing a way off its default, as the bench's readings have it
  bench.values[PITCH] = 0.2;
  let copied = choose(&mut bench, Target::Control(PITCH), Item::Copy);
  assert_eq!(copied, [Action::Copy("-14.40 st".to_owned()), Redraw]);
  assert!(bench.interaction.menu().is_none());
  // onto the formant, which has the same unit: one gesture once the text has come
  assert_eq!(
    choose(&mut bench, Target::Readout(FORMANT), Item::Paste),
    [WantClipboard, Redraw]
  );
  let pasted = bench.send(Event::Clipboard(Some("-14.40 st".to_owned())));
  assert_eq!(pasted[0], Begin(FORMANT));
  assert!(close(set(&pasted[1..2], FORMANT), 0.2));
  assert_eq!(pasted[2..], [End(FORMANT)]);
  // the text was for that once: coming again it does nothing
  assert_eq!(
    bench.send(Event::Clipboard(Some("-14.40 st".to_owned()))),
    []
  );

  assert_eq!(
    choose(&mut bench, Target::Control(MIX), Item::Copy)[0],
    Action::Copy("100%".to_owned())
  );
  choose(&mut bench, Target::Control(MIX), Item::Paste);
  let pasted = bench.send(Event::Clipboard(Some("40%\n".to_owned())));
  assert_eq!(pasted, [Begin(MIX), Set(MIX, 0.4), End(MIX)]);
}

#[test]
fn a_value_that_is_not_the_controls_pastes_as_nothing() {
  let mut bench = Bench::new();
  for text in [
    Some("-14.40 st"),
    Some("hello"),
    Some(""),
    Some("  \n"),
    None,
  ] {
    choose(&mut bench, Target::Control(MIX), Item::Paste);
    let pasted = bench.send(Event::Clipboard(text.map(str::to_owned)));
    assert_eq!(pasted, [], "{text:?}");
  }
  // nor a mix on a shift
  choose(&mut bench, Target::Control(PITCH), Item::Paste);
  assert_eq!(bench.send(Event::Clipboard(Some("40%".to_owned()))), []);
}

#[test]
fn the_menu_opens_on_a_right_press_and_its_rows_act() {
  let mut bench = Bench::new();
  assert_eq!(bench.right(None), []);
  assert_eq!(bench.right(Some(Target::Control(MIX))), [Redraw]);
  assert_eq!(bench.interaction.menu().map(|menu| menu.control), Some(MIX));
  // the row under the pointer is marked, once for each change
  let over = |over| Event::Move {
    at: [0.0, 0.0],
    window: [0.0, 0.0],
    over,
    fine: false,
  };
  assert_eq!(bench.send(over(Some(Target::Item(Item::Reset)))), [Redraw]);
  assert_eq!(bench.send(over(Some(Target::Item(Item::Reset)))), []);
  assert_eq!(bench.interaction.menu().unwrap().hovered, Some(Item::Reset));
  assert_eq!(bench.send(over(Some(Target::Menu))), [Redraw]);
  // a press on the plate between the rows does nothing
  assert_eq!(bench.press(Some(Target::Menu), 100.0), []);
  assert!(bench.interaction.menu().is_some());
  assert_eq!(
    bench.press(Some(Target::Item(Item::Reset)), 100.0),
    [Begin(MIX), Set(MIX, 1.0), End(MIX), Redraw]
  );
  assert!(bench.interaction.menu().is_none());

  bench.right(Some(Target::Readout(PITCH)));
  assert_eq!(
    bench.press(Some(Target::Item(Item::Edit)), 100.0),
    [Focus, Redraw]
  );
  assert_eq!(bench.typed(), ("0.00", 4, true));
}

#[test]
fn a_press_off_the_menu_or_escape_puts_it_away_and_does_nothing_else() {
  let mut bench = Bench::new();
  bench.right(Some(Target::Control(PITCH)));
  assert_eq!(bench.press(Some(Target::Control(MIX)), 100.0), [Redraw]);
  assert!(bench.interaction.menu().is_none());
  // the knob under that press is not being dragged
  assert_eq!(bench.drag(50.0, false), []);
  bench.release();

  bench.right(Some(Target::Control(PITCH)));
  assert_eq!(bench.key(Key::Enter), []);
  assert_eq!(bench.key(Key::Escape), [Redraw]);
  assert!(bench.interaction.menu().is_none());

  bench.right(Some(Target::Control(PITCH)));
  assert_eq!(bench.right(None), [Redraw]);
  assert!(bench.interaction.menu().is_none());
}

/// Creates the bench with its mix made a button that is off.
fn button(push: Push) -> Bench {
  let mut bench = Bench::new();
  bench.interaction.controls[MIX].touch = Touch::Press(push);
  bench.values[MIX] = 0.0;
  bench
}

#[test]
fn a_momentary_button_is_on_for_as_long_as_it_is_held() {
  let mut bench = button(Push::Momentary);
  let over = Some(Target::Control(MIX));
  assert_eq!(bench.press(over, 100.0), [Begin(MIX), Set(MIX, 1.0)]);
  assert_eq!(bench.interaction.held(), Some(MIX));
  // the pointer wandering off while it is held changes nothing
  assert_eq!(bench.drag(20.0, false), []);
  assert_eq!(bench.release(), [Set(MIX, 0.0), End(MIX), Redraw]);
  assert_eq!(bench.interaction.held(), None);
  // held as the window goes, it is let go
  bench.press(over, 100.0);
  assert_eq!(bench.send(Event::Closed), [Set(MIX, 0.0), End(MIX), Redraw]);
}

#[test]
fn a_latching_button_turns_over_each_time_it_is_let_go() {
  let mut bench = button(Push::Latching);
  let over = Some(Target::Control(MIX));
  assert_eq!(bench.press(over, 100.0), [Redraw]);
  assert_eq!(bench.interaction.held(), Some(MIX));
  assert_eq!(
    bench.release(),
    [Begin(MIX), Set(MIX, 1.0), End(MIX), Redraw]
  );
  // a second press straight after is another push, and no reset
  bench.values[MIX] = 1.0;
  assert_eq!(bench.press(over, 100.0), [Redraw]);
  assert_eq!(
    bench.release(),
    [Begin(MIX), Set(MIX, 0.0), End(MIX), Redraw]
  );
  // held as the window goes, it stays as it was
  bench.press(over, 100.0);
  assert_eq!(bench.send(Event::Closed), [Redraw]);
}

#[test]
fn a_button_is_not_turned_by_the_wheel_and_is_reset_with_the_reset_key() {
  let mut bench = button(Push::Latching);
  assert_eq!(bench.wheel(Target::Control(MIX), 1.0, false), []);
  // the mix of the bench is on by default
  assert_eq!(
    bench.reset(Some(Target::Control(MIX))),
    [Begin(MIX), Set(MIX, 1.0), End(MIX)]
  );
  assert_eq!(bench.interaction.held(), None);
}

#[test]
fn a_stepped_switch_goes_one_step_on_with_each_click_and_around_from_the_end() {
  // three positions, like a switch for off, low and high
  let mut bench = Bench::new();
  bench.interaction.dials[MIX] = Dial::linear([0.0, 2.0], 0.0, Some(1.0), 0, "");
  bench.interaction.controls[MIX].touch = Touch::Step;
  let over = Some(Target::Control(MIX));
  let mut click = |from: f32| {
    bench.values[MIX] = from;
    let actions = bench.press(over, 100.0);
    assert_eq!(bench.release(), []);
    match actions[..] {
      [Begin(MIX), Set(MIX, value), End(MIX)] => value,
      _ => panic!("the actions are one whole gesture: {actions:?}"),
    }
  };
  // clicks as close together as a double click are still one step each
  assert_eq!(click(0.0), 0.5);
  assert_eq!(click(0.5), 1.0);
  assert_eq!(click(1.0), 0.0);
}

#[test]
fn a_reset_puts_every_parameter_a_control_turns_at_its_default() {
  // a pad on the pitch and the formant, whose wheel turns the mix
  let mut bench = pad(FORMANT);
  bench.interaction.controls[PITCH].wheel = Some(MIX);
  let all = [
    Begin(PITCH),
    Set(PITCH, 0.5),
    End(PITCH),
    Begin(FORMANT),
    Set(FORMANT, 0.5),
    End(FORMANT),
    Begin(MIX),
    Set(MIX, 1.0),
    End(MIX),
  ];
  let puck = Some(Target::Control(PITCH));
  bench.press(puck, 100.0);
  bench.release();
  assert_eq!(bench.press(puck, 100.0), all);
  bench.release();
  let mut chosen = choose(&mut bench, Target::Control(PITCH), Item::Reset);
  assert_eq!(chosen.pop(), Some(Redraw));
  assert_eq!(chosen, all);
  // the wheel does nothing to a button, so a reset leaves the parameter given its wheel alone
  let mut bench = button(Push::Latching);
  bench.interaction.controls[MIX].wheel = Some(PITCH);
  assert_eq!(
    choose(&mut bench, Target::Control(MIX), Item::Reset),
    [Begin(MIX), Set(MIX, 1.0), End(MIX), Redraw]
  );
}
