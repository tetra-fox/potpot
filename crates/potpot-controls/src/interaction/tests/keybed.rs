use crate::interaction::Action::{NoteOff, NoteOn, Redraw};
use crate::interaction::bench::{Bench, MIX, PITCH};
use crate::interaction::*;

/// Returns the target of a key struck as hard as it can be.
fn key(note: u8) -> Option<Target> {
  Some(Target::Key {
    note,
    velocity: 127,
  })
}

impl Bench {
  /// Moves the pointer onto a target, the buttons as they are.
  fn hover(&mut self, over: Option<Target>) -> Vec<Action> {
    let (at, window) = self.place([100.0, 100.0]);
    self.send(Event::Move {
      at,
      window,
      over,
      fine: false,
    })
  }
}

#[test]
fn a_slide_over_a_keybed_ends_each_note_before_the_next_and_none_twice() {
  let mut bench = Bench::new();
  // a move with no button down plays nothing, and its target is not asked for
  assert!(!bench.interaction.watches_pointer());
  assert_eq!(bench.hover(key(60)), []);
  let soft = Some(Target::Key {
    note: 60,
    velocity: 40,
  });
  let struck = [NoteOn {
    note: 60,
    velocity: 40,
  }];
  assert_eq!(bench.press(soft, 100.0), struck);
  assert!(bench.interaction.watches_pointer());
  assert_eq!(bench.interaction.note(), Some(60));
  assert_eq!(bench.interaction.held(), None);
  // further down the same key is the same note, whatever a press there would strike it with
  assert_eq!(bench.hover(key(60)), []);
  let next = [
    NoteOff { note: 60 },
    NoteOn {
      note: 61,
      velocity: 127,
    },
  ];
  assert_eq!(bench.hover(key(61)), next);
  // off the keys the note is over, and it is over once
  assert_eq!(bench.hover(None), [NoteOff { note: 61 }]);
  assert_eq!(bench.interaction.note(), None);
  assert_eq!(bench.hover(Some(Target::Control(MIX))), []);
  assert!(bench.interaction.watches_pointer());
  // back on them another begins, with the button still down
  let back = [NoteOn {
    note: 64,
    velocity: 127,
  }];
  assert_eq!(bench.hover(key(64)), back);
  assert_eq!(bench.release(), [NoteOff { note: 64 }]);
  assert!(!bench.interaction.watches_pointer());
  // let go off the keys, nothing is left to end
  bench.press(key(60), 100.0);
  bench.hover(None);
  assert_eq!(bench.release(), []);
  assert_eq!(bench.hover(key(62)), []);
}

#[test]
fn a_key_takes_only_the_left_button_and_its_note_ends_with_whatever_ends_the_press() {
  let on = |note| NoteOn {
    note,
    velocity: 127,
  };
  let mut bench = Bench::new();
  // the wheel and the right button do nothing on a key, and no menu comes up
  let over = key(60).expect("a key is a target");
  assert_eq!(bench.wheel(over, 1.0, false), []);
  assert_eq!(bench.right(key(60)), []);
  assert_eq!(bench.interaction.menu(), None);
  // a second press in the time of a double click, and one with the reset key, play as any other
  assert_eq!(bench.press(key(60), 100.0), [on(60)]);
  assert_eq!(bench.release(), [NoteOff { note: 60 }]);
  assert_eq!(bench.press(key(60), 100.0), [on(60)]);
  assert_eq!(bench.release(), [NoteOff { note: 60 }]);
  assert_eq!(bench.reset(key(62)), [on(62)]);
  // a press whose release never came ends the note before the next begins
  assert_eq!(bench.press(key(65), 100.0), [NoteOff { note: 62 }, on(65)]);
  // and so does the window going away
  assert_eq!(bench.send(Event::Closed), [NoteOff { note: 65 }]);
  assert_eq!(bench.release(), []);
  // a key under the menu is not played by the press that takes the menu down
  bench.right(Some(Target::Control(MIX)));
  assert_eq!(bench.press(key(60), 100.0), [Redraw]);
  assert_eq!(bench.interaction.note(), None);
}

#[test]
fn a_note_is_over_when_the_keys_or_the_pointer_go_elsewhere_and_no_move_plays_after() {
  let on = |note| NoteOn {
    note,
    velocity: 127,
  };
  for gone in [Event::Unfocused, Event::Left] {
    let mut bench = Bench::new();
    assert_eq!(bench.press(key(60), 100.0), [on(60)]);
    assert_eq!(bench.send(gone.clone()), [NoteOff { note: 60 }]);
    assert_eq!(bench.interaction.note(), None);
    // the release may never come: a move with no button down plays nothing
    assert!(!bench.interaction.watches_pointer());
    assert_eq!(bench.hover(key(62)), []);
    assert_eq!(bench.release(), []);
    // off the keys with the button down, nothing is left to end and the playing is over
    bench.press(key(60), 100.0);
    bench.hover(None);
    assert_eq!(bench.send(gone), []);
    assert_eq!(bench.hover(key(62)), []);
  }
  // a drag goes on past the window's edge and through the keys going elsewhere
  let mut bench = Bench::new();
  bench.press(Some(Target::Control(PITCH)), 100.0);
  assert_eq!(bench.send(Event::Left), []);
  assert_eq!(bench.send(Event::Unfocused), []);
  assert_eq!(bench.interaction.held(), Some(PITCH));
}

#[test]
fn the_right_button_leaves_a_note_that_is_playing_as_it_is() {
  let mut bench = Bench::new();
  bench.press(key(60), 100.0);
  // on a key, on a control and on nothing: no menu comes up and the note goes on
  for over in [key(64), Some(Target::Control(MIX)), None] {
    assert_eq!(bench.right(over), []);
    assert_eq!(bench.send(Event::Release(Button::Right)), []);
    assert_eq!(bench.interaction.menu(), None);
    assert_eq!(bench.interaction.note(), Some(60));
  }
  let next = [
    NoteOff { note: 60 },
    NoteOn {
      note: 61,
      velocity: 127,
    },
  ];
  assert_eq!(bench.hover(key(61)), next);
  assert_eq!(bench.release(), [NoteOff { note: 61 }]);
}
