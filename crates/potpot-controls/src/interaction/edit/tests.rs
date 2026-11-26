use std::time::{Duration, Instant};

use crate::interaction::Action::{Begin, End, Redraw, Set, WantClipboard};
use crate::interaction::bench::{
  Bench, MIX, PITCH, ROOM, SCALES, SIZE, choose, close, mix, set, shift,
};
use crate::interaction::*;

#[test]
fn each_readout_holds_and_shows_what_its_own_control_says_it_can() {
  // one readout with room for two figures, and one with room for five of anything
  let dials = vec![shift(None), mix()];
  let narrow = Readout {
    room: 2,
    shows: |letter| letter.is_ascii_digit(),
  };
  let wide = Readout {
    room: 5,
    shows: |_| true,
  };
  let (narrow, wide) = (Control::new(0, narrow), Control::new(1, wide));
  let mut panel = Interaction::new(dials, vec![narrow, wide], SIZE, SCALES);
  let values = [0.5, 1.0];
  let mut typed = |control: usize, letters: &str| {
    let moment = Moment {
      now: Instant::now(),
      values: &values,
      scale: 1.0,
    };
    let press = Event::Press {
      button: Button::Left,
      at: [10.0, 10.0],
      window: [10.0, 10.0],
      over: Some(Target::Readout(control)),
      reset: false,
    };
    panel.handle(press.clone(), moment);
    panel.handle(press, moment);
    for letter in letters.chars() {
      panel.handle(Event::Key(Key::Letter(letter)), moment);
    }
    let text = panel.edit().expect("a value is being typed").text.clone();
    panel.handle(Event::Key(Key::Escape), moment);
    text
  };
  assert_eq!(typed(0, "1x2.345"), "12");
  assert_eq!(typed(1, "1x2.345"), "1x2.3");
}

#[test]
fn text_that_is_no_finite_number_sets_nothing() {
  // a dial that reads any number, behind a readout that letters can be typed into
  let mut bench = Bench::new();
  bench.interaction.dials[PITCH] = Dial::linear([-12.0, 12.0], 0.0, None, 1, " st");
  bench.interaction.controls[PITCH].readout = Readout::new(ROOM);
  bench.edit(PITCH);
  bench.kind("NAN");
  assert_eq!(bench.typed().0, "NAN");
  assert_eq!(bench.key(Key::Enter), [Redraw]);
  choose(&mut bench, Target::Control(PITCH), Item::Paste);
  assert_eq!(bench.send(Event::Clipboard(Some("nan".to_owned()))), []);
  // past the end of the turn is still its end
  bench.after(Duration::from_secs(1), Event::Tick);
  bench.edit(PITCH);
  bench.kind("inf");
  assert_eq!(
    bench.key(Key::Enter),
    [Begin(PITCH), Set(PITCH, 1.0), End(PITCH), Redraw]
  );
}

#[test]
fn typing_replaces_the_selected_value_and_enter_sets_it() {
  let mut bench = Bench::new();
  bench.edit(PITCH);
  assert_eq!(bench.typed(), ("0.00", 4, true));
  assert!(bench.interaction.takes_keys());
  bench.kind("12");
  assert_eq!(bench.typed(), ("12", 2, false));
  // twelve semitones up is three quarters of the way from 24 down to 24 up
  assert_eq!(
    bench.key(Key::Enter),
    [Begin(PITCH), Set(PITCH, 0.75), End(PITCH), Redraw]
  );
  assert!(bench.interaction.edit().is_none() && !bench.interaction.takes_keys());
}

#[test]
fn the_caret_moves_and_keys_change_the_text_at_it() {
  let mut bench = Bench::new();
  bench.edit(PITCH);
  // an arrow lets go of the selection at that end of it
  bench.key(Key::Left { extend: false });
  assert_eq!(bench.typed(), ("0.00", 0, false));
  bench.key(Key::Left { extend: false });
  assert_eq!(bench.typed(), ("0.00", 0, false));
  bench.kind("-1");
  assert_eq!(bench.typed(), ("-10.00", 2, false));
  bench.key(Key::Delete);
  assert_eq!(bench.typed(), ("-1.00", 2, false));
  bench.key(Key::End { extend: false });
  bench.key(Key::Backspace);
  bench.key(Key::Backspace);
  assert_eq!(bench.typed(), ("-1.", 3, false));
  bench.key(Key::Right { extend: false });
  assert_eq!(bench.typed(), ("-1.", 3, false));
  bench.key(Key::Home { extend: false });
  bench.key(Key::Backspace);
  assert_eq!(bench.typed(), ("-1.", 0, false));
  bench.key(Key::End { extend: false });
  bench.key(Key::Delete);
  bench.kind("5 st");
  assert_eq!(bench.typed(), ("-1.5 st", 7, false));
  let actions = bench.key(Key::Enter);
  assert!(close(set(&actions[1..2], PITCH), (24.0 - 1.5) / 48.0));
}

#[test]
fn a_value_is_read_with_a_sign_a_comma_or_its_unit() {
  let mut bench = Bench::new();
  for (control, typed, value) in [
    (PITCH, "+12", 0.75),
    (PITCH, "-9,5", (24.0 - 9.5) / 48.0),
    (PITCH, "-9.5 st", (24.0 - 9.5) / 48.0),
    (PITCH, "7", 31.0 / 48.0),
    (MIX, "75%", 0.75),
    (MIX, "75 %", 0.75),
    (MIX, "7,5", 0.075),
    (MIX, "40", 0.4),
  ] {
    bench.after(Duration::from_secs(1), Event::Tick);
    bench.edit(control);
    bench.kind(typed);
    let actions = bench.key(Key::Enter);
    assert!(
      close(set(&actions[1..2], control), value),
      "{typed}: {actions:?}"
    );
  }
}

#[test]
fn tab_sets_what_was_typed_and_select_all_lets_it_be_typed_over() {
  let mut bench = Bench::new();
  bench.edit(MIX);
  bench.kind("12");
  assert_eq!(bench.key(Key::SelectAll), [Redraw]);
  assert_eq!(bench.typed(), ("12", 2, true));
  bench.kind("30");
  assert_eq!(bench.typed(), ("30", 2, false));
  assert_eq!(
    bench.key(Key::Tab),
    [Begin(MIX), Set(MIX, 0.3), End(MIX), Redraw]
  );
  assert!(!bench.interaction.takes_keys());
}

#[test]
fn the_arrows_with_extend_select_from_where_the_caret_was() {
  let mut bench = Bench::new();
  bench.edit(PITCH);
  bench.kind("-12.5");
  let selected = |bench: &Bench| bench.interaction.edit().unwrap().selected();
  bench.key(Key::Left { extend: true });
  bench.key(Key::Left { extend: true });
  assert_eq!((selected(&bench), bench.typed().1), (3..5, 3));
  // a step back toward where it began makes the selection a letter shorter
  bench.key(Key::Right { extend: true });
  assert_eq!(selected(&bench), 4..5);
  bench.key(Key::Home { extend: true });
  assert_eq!((selected(&bench), bench.typed().1), (0..5, 0));
  // an arrow alone lets go of it at that end
  bench.key(Key::Right { extend: false });
  assert_eq!((selected(&bench), bench.typed().1), (5..5, 5));
  bench.key(Key::Home { extend: false });
  bench.key(Key::Right { extend: false });
  bench.key(Key::End { extend: true });
  assert_eq!(selected(&bench), 1..5);
  // typing takes the selection's place, and so does a backspace take it away
  bench.kind("7");
  assert_eq!(bench.typed(), ("-7", 2, false));
  bench.key(Key::Left { extend: true });
  bench.key(Key::Backspace);
  assert_eq!(bench.typed(), ("-", 1, false));
}

#[test]
fn copy_takes_the_selection_or_all_of_it_and_cut_takes_it_out() {
  let mut bench = Bench::new();
  bench.edit(PITCH);
  // all of it is selected to begin with
  assert_eq!(bench.key(Key::Copy), [Action::Copy("0.00".to_owned())]);
  bench.kind("-12.5");
  // with nothing selected all of it is copied, and nothing about it changes
  assert_eq!(bench.key(Key::Copy), [Action::Copy("-12.5".to_owned())]);
  assert_eq!(bench.typed(), ("-12.5", 5, false));
  bench.key(Key::Left { extend: true });
  bench.key(Key::Left { extend: true });
  assert_eq!(bench.key(Key::Copy), [Action::Copy(".5".to_owned())]);
  assert_eq!(bench.typed(), ("-12.5", 3, true));
  assert_eq!(bench.key(Key::Cut), [Action::Copy(".5".to_owned()), Redraw]);
  assert_eq!(bench.typed(), ("-12", 3, false));
  assert_eq!(
    bench.key(Key::Cut),
    [Action::Copy("-12".to_owned()), Redraw]
  );
  assert_eq!(bench.typed(), ("", 0, false));
}

#[test]
fn paste_puts_the_clipboards_text_in_place_of_the_selection() {
  let mut bench = Bench::new();
  bench.edit(PITCH);
  bench.kind("-12.5");
  bench.key(Key::Home { extend: false });
  bench.key(Key::Right { extend: false });
  bench.key(Key::Right { extend: true });
  bench.key(Key::Right { extend: true });
  // the text is asked for, and nothing changes until it comes
  assert_eq!(bench.key(Key::Paste), [WantClipboard]);
  assert_eq!(bench.typed(), ("-12.5", 3, true));
  // it comes with space and a line end around it, which are left off
  let pasted = bench.send(Event::Clipboard(Some("  7 \n".to_owned())));
  assert_eq!(pasted, [Redraw]);
  assert_eq!(bench.typed(), ("-7.5", 2, false));
  // and then is what Enter sets
  let actions = bench.key(Key::Enter);
  assert!(close(set(&actions[1..2], PITCH), (24.0 - 7.5) / 48.0));
}

#[test]
fn pasted_text_is_only_what_the_readout_shows_and_as_much_as_fits() {
  let mut bench = Bench::new();
  bench.edit(PITCH);
  bench.key(Key::Paste);
  // letters the bench's readout has none for fall out, and the rest stops at its room
  bench.send(Event::Clipboard(Some(
    "a1\u{e9}b2\n3#4 st 56789".to_owned(),
  )));
  assert_eq!(bench.typed(), ("1234 st 5", ROOM, false));
  // into a full readout nothing goes, but a selection still makes room
  bench.key(Key::Paste);
  bench.send(Event::Clipboard(Some("00".to_owned())));
  assert_eq!(bench.typed(), ("1234 st 5", ROOM, false));
  bench.key(Key::Left { extend: true });
  bench.key(Key::Paste);
  bench.send(Event::Clipboard(Some("678".to_owned())));
  assert_eq!(bench.typed(), ("1234 st 6", ROOM, false));
}

#[test]
fn clipboard_text_nobody_is_waiting_for_goes_nowhere() {
  let mut bench = Bench::new();
  assert_eq!(bench.send(Event::Clipboard(Some("12".to_owned()))), []);
  bench.edit(PITCH);
  // unasked for
  assert_eq!(bench.send(Event::Clipboard(Some("12".to_owned()))), []);
  assert_eq!(bench.typed(), ("0.00", 4, true));
  // a clipboard with no text
  bench.key(Key::Paste);
  assert_eq!(bench.send(Event::Clipboard(None)), []);
  assert_eq!(bench.typed(), ("0.00", 4, true));
  // asked for, but typing ended before it came, and then began on another control
  bench.key(Key::Paste);
  bench.key(Key::Escape);
  bench.after(Duration::from_secs(1), Event::Tick);
  bench.edit(MIX);
  assert_eq!(bench.send(Event::Clipboard(Some("12".to_owned()))), []);
  assert_eq!(bench.typed(), ("100", 3, true));
}

#[test]
fn a_selected_value_is_cleared_by_one_backspace() {
  let mut bench = Bench::new();
  bench.edit(MIX);
  assert_eq!(bench.typed(), ("100", 3, true));
  bench.key(Key::Backspace);
  assert_eq!(bench.typed(), ("", 0, false));
  bench.key(Key::Backspace);
  bench.key(Key::Delete);
  assert_eq!(bench.typed(), ("", 0, false));
  bench.kind("25%");
  assert_eq!(
    bench.key(Key::Enter),
    [Begin(MIX), Set(MIX, 0.25), End(MIX), Redraw]
  );
}

#[test]
fn only_what_the_readout_can_show_is_typed_and_only_as_much_as_fits() {
  let mut bench = Bench::new();
  bench.edit(PITCH);
  // a key the readout has no letter for leaves the selection as it was
  bench.kind("\u{e9}#");
  assert_eq!(bench.typed(), ("0.00", 4, true));
  bench.kind("123456789012");
  assert_eq!(bench.typed().0.len(), ROOM);
  assert_eq!(bench.typed().0, &"123456789012"[..ROOM]);
}

#[test]
fn escape_and_text_that_is_no_value_leave_the_value_alone() {
  let mut bench = Bench::new();
  bench.edit(PITCH);
  bench.kind("7");
  assert_eq!(bench.key(Key::Escape), [Redraw]);
  assert!(bench.interaction.edit().is_none());
  bench.after(Duration::from_secs(1), Event::Tick);
  bench.edit(PITCH);
  bench.kind("1-.");
  assert_eq!(bench.key(Key::Enter), [Redraw]);
  assert!(bench.interaction.edit().is_none());
}

#[test]
fn a_press_elsewhere_sets_what_was_typed_and_then_counts_as_itself() {
  let mut bench = Bench::new();
  bench.edit(PITCH);
  bench.kind("24");
  // on the readout being typed in it changes nothing
  assert_eq!(bench.press(Some(Target::Readout(PITCH)), 100.0), []);
  assert_eq!(bench.typed(), ("24", 2, false));
  bench.release();
  bench.after(Duration::from_secs(1), Event::Tick);
  assert_eq!(
    bench.press(Some(Target::Control(MIX)), 100.0),
    [Begin(PITCH), Set(PITCH, 1.0), End(PITCH), Redraw]
  );
  assert_eq!(bench.drag(120.0, false)[0], Begin(MIX));
}

#[test]
fn losing_the_keys_sets_what_was_typed() {
  let mut bench = Bench::new();
  bench.edit(MIX);
  bench.kind("50");
  assert_eq!(
    bench.send(Event::Unfocused),
    [Begin(MIX), Set(MIX, 0.5), End(MIX), Redraw]
  );
  assert_eq!(bench.send(Event::Unfocused), []);
}
