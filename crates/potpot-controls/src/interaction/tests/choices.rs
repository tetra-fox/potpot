use std::time::Instant;

use crate::interaction::Action::{Begin, End, Redraw, Set};
use crate::interaction::*;

// a selector of four, a stepper of one to sixteen, and a smooth control, each on its own
// parameter and in that order
const SELECTOR: usize = 0;
const STEPPER: usize = 1;
const SMOOTH: usize = 2;

/// A panel of a selector, a stepper and a smooth control.
///
/// It holds where each stands.
struct Panel {
  interaction: Interaction,
  values: [f32; 3],
}

impl Panel {
  /// Creates the panel, on a hidden page if `hidden`.
  fn new(hidden: bool) -> Self {
    let dials = vec![
      Dial::linear([0.0, 3.0], 0.0, Some(1.0), 0, ""),
      Dial::linear([1.0, 16.0], 8.0, Some(1.0), 0, ""),
      Dial::linear([0.0, 1.0], 0.5, None, 2, ""),
    ];
    let mut controls: Vec<Control> = (0..3)
      .map(|parameter| Control::new(parameter, Readout::new(4)))
      .collect();
    if hidden {
      for control in &mut controls {
        control.page = 1;
      }
    }
    Self {
      interaction: Interaction::paged(dials, controls, [200.0, 100.0], [1.0, 1.0], 2, 0),
      values: [0.0, 7.0 / 15.0, 0.5],
    }
  }

  fn send(&mut self, event: Event) -> Vec<Action> {
    let moment = Moment {
      now: Instant::now(),
      values: &self.values,
      scale: 1.0,
    };
    self.interaction.handle(event, moment)
  }

  /// Presses the left button on a target and lets it go.
  fn click(&mut self, over: Target) -> Vec<Action> {
    let pressed = self.send(Event::Press {
      button: Button::Left,
      at: [10.0, 10.0],
      window: [10.0, 10.0],
      over: Some(over),
      reset: false,
    });
    self.send(Event::Release(Button::Left));
    pressed
  }

  /// Presses an arrow and lets it go.
  ///
  /// Returns what the press did before the arrow went in.
  fn step(&mut self, arrow: Target) -> Vec<Action> {
    let mut pressed = self.click(arrow);
    assert_eq!(pressed.pop(), Some(Redraw), "the arrow goes in");
    pressed
  }

  /// Takes the value a whole gesture sets a parameter to.
  fn take(&mut self, parameter: usize, actions: &[Action]) -> f32 {
    let [Begin(begun), Set(set, value), End(ended)] = actions else {
      panic!("the actions are one whole gesture: {actions:?}");
    };
    assert_eq!([*begun, *set, *ended], [parameter; 3]);
    self.values[parameter] = *value;
    *value
  }
}

/// Returns a choice of the selector.
fn choice(choice: usize) -> Target {
  Target::Choice {
    control: SELECTOR,
    choice,
  }
}

#[test]
fn a_choice_puts_its_control_that_many_steps_from_the_start() {
  let mut panel = Panel::new(false);
  for (pick, value) in [(2, 2.0 / 3.0), (3, 1.0), (0, 0.0), (1, 1.0 / 3.0)] {
    let actions = panel.click(choice(pick));
    assert!(
      (panel.take(SELECTOR, &actions) - value).abs() < 1e-6,
      "{pick}"
    );
  }
  // the choice it stands at already sets nothing, and one past the end is the end
  assert_eq!(panel.click(choice(1)), []);
  let actions = panel.click(choice(7));
  assert_eq!(panel.take(SELECTOR, &actions), 1.0);
  // a control with no step takes one of its plain units for each
  let actions = panel.click(Target::Choice {
    control: SMOOTH,
    choice: 1,
  });
  assert_eq!(panel.take(SMOOTH, &actions), 1.0);
}

#[test]
fn a_stepper_moves_one_step_each_press_and_stops_at_either_end() {
  let mut panel = Panel::new(false);
  let at = |panel: &Panel| (panel.values[STEPPER] * 15.0 + 1.0).round();
  let actions = panel.step(Target::Increment(STEPPER));
  panel.take(STEPPER, &actions);
  assert_eq!(at(&panel), 9.0);
  // a quick second press is one more step, and no double click
  for _ in 0..7 {
    let actions = panel.step(Target::Increment(STEPPER));
    panel.take(STEPPER, &actions);
  }
  assert_eq!((at(&panel), panel.values[STEPPER]), (16.0, 1.0));
  assert_eq!(panel.step(Target::Increment(STEPPER)), []);
  for _ in 0..15 {
    let actions = panel.step(Target::Decrement(STEPPER));
    panel.take(STEPPER, &actions);
  }
  assert_eq!((at(&panel), panel.values[STEPPER]), (1.0, 0.0));
  assert_eq!(panel.step(Target::Decrement(STEPPER)), []);
  // from between two steps the first press goes to the next one that way
  panel.values[STEPPER] = 4.5 / 15.0;
  let actions = panel.step(Target::Increment(STEPPER));
  panel.take(STEPPER, &actions);
  assert_eq!(at(&panel), 6.0);
}

#[test]
fn an_arrow_is_held_from_its_press_to_its_release_and_steps_once() {
  let mut panel = Panel::new(false);
  let press = |over| Event::Press {
    button: Button::Left,
    at: [10.0, 10.0],
    window: [10.0, 10.0],
    over: Some(over),
    reset: false,
  };
  let on = Target::Increment(STEPPER);
  let pressed = panel.send(press(on));
  assert_eq!(
    pressed,
    [
      Begin(STEPPER),
      Set(STEPPER, 8.0 / 15.0),
      End(STEPPER),
      Redraw
    ]
  );
  assert_eq!(panel.interaction.arrow(), Some(on));
  assert_eq!(panel.interaction.held(), Some(STEPPER));
  // held, it moves nothing more, and let go it comes back out
  assert_eq!(panel.send(Event::Tick), []);
  assert_eq!(panel.send(Event::Release(Button::Left)), [Redraw]);
  assert_eq!(panel.interaction.arrow(), None);
  // a press elsewhere with no release before it lets it out, as does a press on a tab
  panel.values[STEPPER] = 8.0 / 15.0;
  let back = Target::Decrement(STEPPER);
  panel.send(press(back));
  assert_eq!(panel.interaction.arrow(), Some(back));
  assert_eq!(panel.send(press(Target::Control(SMOOTH))), [Redraw]);
  assert_eq!(panel.interaction.arrow(), None);
  panel.send(Event::Release(Button::Left));
  panel.send(press(back));
  assert_eq!(panel.send(press(Target::Page(1))), [Redraw, Redraw]);
  assert_eq!(
    (panel.interaction.arrow(), panel.interaction.page()),
    (None, 1)
  );
}

#[test]
fn the_wheel_and_the_menu_work_on_a_choice_or_an_arrow_as_on_its_control() {
  let mut panel = Panel::new(false);
  let wheel = |over| Event::Wheel {
    over: Some(over),
    lines: 1.0,
    fine: false,
  };
  let turned = panel.send(wheel(choice(3)));
  assert_eq!(turned, [Begin(SELECTOR), Set(SELECTOR, 1.0 / 3.0)]);
  panel.send(Event::Closed);
  let turned = panel.send(wheel(Target::Decrement(STEPPER)));
  // the wheel turns the control as it turns, whichever arrow it is over
  assert_eq!(turned, [Begin(STEPPER), Set(STEPPER, 8.0 / 15.0)]);
  panel.send(Event::Closed);
  let right = |over| Event::Press {
    button: Button::Right,
    at: [10.0, 10.0],
    window: [10.0, 10.0],
    over: Some(over),
    reset: false,
  };
  assert_eq!(panel.send(right(choice(2))), [Redraw]);
  assert_eq!(
    panel.interaction.menu().map(|menu| menu.control),
    Some(SELECTOR)
  );
}

#[test]
fn a_choice_or_an_arrow_on_a_page_not_shown_takes_no_input() {
  let mut panel = Panel::new(true);
  for over in [
    choice(2),
    Target::Increment(STEPPER),
    Target::Decrement(STEPPER),
  ] {
    assert_eq!(panel.click(over), []);
    let wheel = Event::Wheel {
      over: Some(over),
      lines: 1.0,
      fine: false,
    };
    assert_eq!(panel.send(wheel), []);
  }
}
