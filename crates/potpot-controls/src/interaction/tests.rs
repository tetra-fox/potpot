use super::Action::{Begin, End, Focus, Redraw, Set, WantClipboard};
use super::*;

/// how many letters the readouts of the bench have room for
const ROOM: usize = 9;

/// a panel with every control at its default, the shifts in the middle of their turn and
/// the mix at the end of its own, and events arriving a hundredth of a second apart
struct Bench {
  interaction: Interaction,
  /// where the controls stand, which is their defaults unless a test says otherwise
  values: [f32; 4],
  /// how many times its own size the window is
  scale: f32,
  now: Instant,
}

/// the bench's controls: two shifts of 24 semitones either way and a mix in hundredths,
/// all three turning smoothly, and a shift like the others that moves in whole semitones
const PITCH: usize = 0;
const FORMANT: usize = 1;
const MIX: usize = 2;
const STEPPED: usize = 3;
const SIZE: Point = [500.0, 250.0];
const SCALES: [f32; 2] = [0.75, 3.0];

fn shift(step: Option<f32>) -> Dial {
  Dial {
    default: 0.5,
    step,
    plain: Box::new(|value| value * 48.0 - 24.0),
    turn: Box::new(|plain| ((plain + 24.0) / 48.0).clamp(0.0, 1.0)),
    write: Box::new(|value| format!("{:.2}", value * 48.0 - 24.0)),
    unit: " st".to_owned(),
    read: Box::new(|text| {
      let plain: f32 = text.trim().trim_end_matches(" st").parse().ok()?;
      Some(((plain + 24.0) / 48.0).clamp(0.0, 1.0))
    }),
  }
}

fn mix() -> Dial {
  Dial {
    default: 1.0,
    step: None,
    plain: Box::new(|value| value),
    turn: Box::new(|plain| plain.clamp(0.0, 1.0)),
    write: Box::new(|value| format!("{:.0}", value * 100.0)),
    unit: "%".to_owned(),
    read: Box::new(|text| {
      let hundredths: f32 = text.trim().trim_end_matches([' ', '%']).parse().ok()?;
      Some((hundredths / 100.0).clamp(0.0, 1.0))
    }),
  }
}

#[test]
fn a_control_turns_the_parameter_it_names_and_not_the_one_at_its_own_place() {
  // two controls on four parameters: the first turns the mix, the second the pitch
  let dials = vec![shift(None), shift(None), mix(), shift(Some(1.0))];
  let on = |parameter| Control {
    parameter,
    feel: Feel::default(),
    readout: Readout {
      room: ROOM,
      shows: |_| true,
    },
    touch: Touch::Turn,
  };
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
  // a double click on the second control resets the pitch, which is the first parameter
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
fn each_readout_holds_and_shows_what_its_own_control_says_it_can() {
  // one readout with room for two figures, and one with room for five of anything
  let dials = vec![shift(None), mix()];
  let narrow = Control {
    parameter: 0,
    feel: Feel::default(),
    readout: Readout {
      room: 2,
      shows: |letter| letter.is_ascii_digit(),
    },
    touch: Touch::Turn,
  };
  let wide = Control {
    parameter: 1,
    feel: Feel::default(),
    readout: Readout {
      room: 5,
      shows: |_| true,
    },
    touch: Touch::Turn,
  };
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
    let text = panel.edit().expect("a value being typed").text.clone();
    panel.handle(Event::Key(Key::Escape), moment);
    text
  };
  assert_eq!(typed(0, "1x2.345"), "12");
  assert_eq!(typed(1, "1x2.345"), "1x2.3");
}

#[test]
fn a_linear_dial_is_written_and_read_in_its_plain_units() {
  let dial = Dial::linear([-12.0, 12.0], 0.0, Some(1.0), 1, " st");
  assert_eq!(dial.default, 0.5);
  assert_eq!((dial.plain)(0.75), 6.0);
  assert_eq!((dial.turn)(-6.0), 0.25);
  assert_eq!(
    ((dial.write)(0.75), dial.unit.as_str()),
    ("6.0".to_owned(), " st")
  );
  assert_eq!((dial.write)(0.25), "-6.0");
  // read with the unit or without, and with room around either
  for text in ["6", "6.0 st", " 6 st ", "6st"] {
    assert_eq!((dial.read)(text), Some(0.75), "{text}");
  }
  // past an end is that end, and what is not a number is nothing
  assert_eq!((dial.read)("40"), Some(1.0));
  assert_eq!((dial.read)("-40 st"), Some(0.0));
  assert_eq!((dial.read)("six"), None);
  assert_eq!((dial.read)(""), None);
}

fn interaction() -> Interaction {
  let dials = vec![shift(None), shift(None), mix(), shift(Some(1.0))];
  // a control for each parameter, in the same order, with readouts all alike
  let controls = (0..dials.len())
    .map(|parameter| Control {
      parameter,
      feel: Feel::default(),
      readout: Readout {
        room: ROOM,
        shows: |letter| letter.is_ascii_digit() || " +-.,%st".contains(letter),
      },
      touch: Touch::Turn,
    })
    .collect();
  Interaction::new(dials, controls, SIZE, SCALES)
}

impl Bench {
  fn new() -> Self {
    Self {
      interaction: interaction(),
      values: [0.5, 0.5, 1.0, 0.5],
      scale: 1.0,
      now: Instant::now(),
    }
  }

  fn after(&mut self, wait: Duration, event: Event) -> Vec<Action> {
    self.now += wait;
    let moment = Moment {
      now: self.now,
      values: &self.values,
      scale: self.scale,
    };
    self.interaction.handle(event, moment)
  }

  fn send(&mut self, event: Event) -> Vec<Action> {
    self.after(Duration::from_millis(10), event)
  }

  /// a place on the panel, and where that is in the window at the bench's scale
  fn place(&self, at: Point) -> (Point, Point) {
    (at, at.map(|across| across * self.scale))
  }

  fn press(&mut self, over: Option<Target>, y: f32) -> Vec<Action> {
    let (at, window) = self.place([100.0, y]);
    self.send(Event::Press {
      button: Button::Left,
      at,
      window,
      over,
      reset: false,
    })
  }

  fn right(&mut self, over: Option<Target>) -> Vec<Action> {
    let (at, window) = self.place([100.0, 100.0]);
    self.send(Event::Press {
      button: Button::Right,
      at,
      window,
      over,
      reset: false,
    })
  }

  fn drag(&mut self, y: f32, fine: bool) -> Vec<Action> {
    let (at, window) = self.place([100.0, y]);
    self.send(Event::Move {
      at,
      window,
      over: None,
      fine,
    })
  }

  fn release(&mut self) -> Vec<Action> {
    self.send(Event::Release(Button::Left))
  }

  fn wheel(&mut self, over: Target, lines: f32, fine: bool) -> Vec<Action> {
    self.send(Event::Wheel {
      over: Some(over),
      lines,
      fine,
    })
  }

  fn key(&mut self, key: Key) -> Vec<Action> {
    self.send(Event::Key(key))
  }

  fn kind(&mut self, text: &str) {
    for letter in text.chars() {
      self.key(Key::Letter(letter));
    }
  }

  /// a double click on a readout, which begins typing its value
  fn edit(&mut self, control: usize) {
    let readout = Some(Target::Readout(control));
    assert_eq!(self.press(readout, 100.0), []);
    self.release();
    assert_eq!(self.press(readout, 100.0), [Focus, Redraw]);
    self.release();
  }

  fn typed(&self) -> (&str, usize, bool) {
    let edit = self.interaction.edit().expect("a value being typed");
    (&edit.text, edit.caret, !edit.selected().is_empty())
  }
}

/// the value a list of actions sets, which has to be the only thing they do
fn set(actions: &[Action], control: usize) -> f32 {
  match actions {
    [Set(set, value)] if *set == control => *value,
    other => panic!("not one value for {control:?}: {other:?}"),
  }
}

fn close(a: f32, b: f32) -> bool {
  (a - b).abs() < 1e-5
}

#[test]
fn a_drag_is_one_gesture_from_the_first_move_to_the_release() {
  let mut bench = Bench::new();
  assert_eq!(bench.press(Some(Target::Control(PITCH)), 100.0), []);
  let first = bench.drag(80.0, false);
  assert_eq!(first[0], Begin(PITCH));
  assert!(close(set(&first[1..], PITCH), 0.6));
  assert!(close(set(&bench.drag(90.0, false), PITCH), 0.55));
  assert_eq!(bench.release(), [End(PITCH)]);
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

/// the value of the bench's shifts that is so many semitones
fn semitones(plain: f32) -> f32 {
  (plain + 24.0) / 48.0
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

/// the grip, in the far corner of the bench's panel, ten points in from both edges
const GRIP: Point = [SIZE[0] - 10.0, SIZE[1] - 10.0];

/// the pointer moved to a place in the window
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
    _ => panic!("not one scale: {actions:?}"),
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
  // the window becomes about that size, but the panel fitted into its whole pixels is a
  // hair wider or narrower, so the same place in the window is a little way off on the
  // panel. the host then moves the pointer to where it already is
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
  // at a window half as large again, held three points off the grip's middle
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
fn a_double_click_on_a_knob_resets_it() {
  let mut bench = Bench::new();
  let knob = Some(Target::Control(MIX));
  bench.press(knob, 100.0);
  bench.release();
  assert_eq!(
    bench.press(knob, 101.0),
    [Begin(MIX), Set(MIX, 1.0), End(MIX)]
  );
  // the reset is whole, so the release ends nothing and a drag does not follow
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
  let actions = bench.send(Event::Press {
    button: Button::Left,
    at: [100.0, 100.0],
    window: [100.0, 100.0],
    over: Some(Target::Control(PITCH)),
    reset: true,
  });
  assert_eq!(actions, [Begin(PITCH), Set(PITCH, 0.5), End(PITCH)]);
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
fn a_press_while_the_grip_is_pulled_lets_the_grip_go_first() {
  let mut bench = Bench::new();
  grab(&mut bench);
  assert_eq!(bench.press(None, 100.0), []);
  // with no button down on the grip the pointer scales nothing
  assert_eq!(pull(&mut bench, [GRIP[0] + 50.0, GRIP[1]]), []);
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
fn a_drag_and_the_wheel_start_from_where_the_control_is_shown() {
  // a value set a moment ago may not have reached the parameter, which still has its default
  let mut interaction = interaction();
  let moment = Moment {
    now: Instant::now(),
    values: &[0.2, 0.5, 1.0, 0.5],
    scale: 1.0,
  };
  let press = Event::Press {
    button: Button::Left,
    at: [100.0, 100.0],
    window: [100.0, 100.0],
    over: Some(Target::Control(PITCH)),
    reset: false,
  };
  interaction.handle(press, moment);
  let moved = Event::Move {
    at: [100.0, 80.0],
    window: [100.0, 80.0],
    over: None,
    fine: false,
  };
  let actions = interaction.handle(moved, moment);
  assert!(close(set(&actions[1..], PITCH), 0.3));
  interaction.handle(Event::Release(Button::Left), moment);
  let wheel = Event::Wheel {
    over: Some(Target::Control(PITCH)),
    lines: 1.0,
    fine: false,
  };
  let actions = interaction.handle(wheel, moment);
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

/// the menu opened on something and one of its rows pressed
fn choose(bench: &mut Bench, on: Target, item: Item) -> Vec<Action> {
  bench.after(Duration::from_secs(1), Event::Tick);
  assert_eq!(bench.right(Some(on)), [Redraw]);
  bench.press(Some(Target::Item(item)), 100.0)
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
  // back over where it began, the selection turns around
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

/// the bench with its mix made a fader that slides to the right, a hundred points for
/// its whole turn, and standing in the middle
fn fader() -> Bench {
  let mut bench = Bench::new();
  bench.interaction.controls[MIX].feel.drag = Drag::Along {
    way: [1.0, 0.0],
    turn: 0.01,
  };
  bench.values[MIX] = 0.5;
  bench
}

fn slide(bench: &mut Bench, at: Point) -> Vec<Action> {
  let (at, window) = bench.place(at);
  bench.send(Event::Move {
    at,
    window,
    over: None,
    fine: false,
  })
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

/// the bench with its mix made a button of a kind, which is off
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
  let (at, window) = bench.place([100.0, 100.0]);
  let reset = Event::Press {
    button: Button::Left,
    at,
    window,
    over: Some(Target::Control(MIX)),
    reset: true,
  };
  // the mix of the bench is on by default
  assert_eq!(bench.send(reset), [Begin(MIX), Set(MIX, 1.0), End(MIX)]);
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
      _ => panic!("not one whole gesture: {actions:?}"),
    }
  };
  // clicks as close together as a double click are still one step each
  assert_eq!(click(0.0), 0.5);
  assert_eq!(click(0.5), 1.0);
  assert_eq!(click(1.0), 0.0);
}

#[test]
fn a_latching_button_stays_in_and_a_momentary_one_comes_back() {
  // held, either is all the way in
  assert_eq!(Push::Latching.depth(false, true), 1.0);
  assert_eq!(Push::Momentary.depth(false, true), 1.0);
  // let go, a latching one that is on stays most of the way in and the rest are out
  let latched = Push::Latching.depth(true, false);
  assert!(latched > 0.5 && latched < 1.0);
  assert_eq!(Push::Latching.depth(false, false), 0.0);
  assert_eq!(Push::Momentary.depth(true, false), 0.0);
  // each push turns a latching one over
  assert!(Push::Latching.released(false));
  assert!(!Push::Latching.released(true));
  assert!(!Push::Momentary.released(false));
}
