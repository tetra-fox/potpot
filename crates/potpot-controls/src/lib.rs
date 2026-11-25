//! what the controls of an audio plugin's panel do, apart from how they look, what window
//! they are in and what plugin they belong to. nothing here draws, and nothing here depends
//! on anything: a panel's look is built on top, with whatever it is drawn by.
//!
//! [`interaction`] is the heart of it: events go in, and what to tell the host and the
//! window comes out.
//!
//! ```
//! use std::time::Instant;
//!
//! use potpot_controls::interaction::{
//!   Action, Button, Control, Dial, Event, Interaction, Moment, Readout, Target,
//! };
//!
//! // a parameter over two octaves in whole semitones, and one knob that turns it, on a panel
//! // 200 points by 100
//! let dial = Dial::linear([-12.0, 12.0], 0.0, Some(1.0), 0, " st");
//! let knob = Control::new(0, Readout::new(3));
//! let mut panel = Interaction::new(vec![dial], vec![knob], [200.0, 100.0], [1.0, 2.0]);
//!
//! // the pointer goes down on it and up the panel by forty points
//! let moment = Moment { now: Instant::now(), values: &[0.5], scale: 1.0 };
//! let press = Event::Press {
//!   button: Button::Left,
//!   at: [50.0, 60.0],
//!   window: [50.0, 60.0],
//!   over: Some(Target::Control(0)),
//!   reset: false,
//! };
//! panel.handle(press, moment);
//! let moved = Event::Move { at: [50.0, 20.0], window: [50.0, 20.0], over: None, fine: false };
//! let actions = panel.handle(moved, moment);
//!
//! // the host is to hear that a gesture began, and the value it has reached
//! assert_eq!(actions[0], Action::Begin(0));
//! assert!(matches!(actions[1], Action::Set(0, value) if value > 0.5));
//! ```
//!
//! the rest is what a window around it needs whatever it draws with: [`sent`] for showing
//! values a host has yet to take, [`fit`] for a panel of one shape in windows of any,
//! [`furniture`] for the menu and the grip, and [`flat`] and [`stroke`] for plain shapes and
//! lettering to draw them in

/// a place or a size, across and down
pub type Point = [f32; 2];

pub mod fit;
pub mod flat;
pub mod furniture;
pub mod interaction;
pub mod sent;
pub mod stroke;
