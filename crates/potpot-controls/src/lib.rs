//! What an audio plugin panel's controls do and its displays plot, with no look.
//!
//! [`Interaction`](interaction::Interaction) takes pointer and key events and returns the
//! [`Action`](interaction::Action)s for the host and the window. The crate draws nothing and
//! depends on nothing: a panel's look is built on top, with whatever it is drawn by.
//!
//! # Examples
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
//! # Window support
//!
//! - [`sent`] shows values a host has yet to take.
//! - [`fit`] puts a panel of one shape in windows of any.
//! - [`furniture`] holds the menu and the grip.
//! - [`flat`] and [`stroke`] hold plain shapes and lettering to draw them in.
//!
//! # Displays
//!
//! - [`axis`] lays plain values along a line, evenly or by ratio, for a parameter and for a
//!   drawing of it alike.
//! - [`response`] gives the gain of an equalizer's bands over frequency.
//! - [`spectrum`] gathers the bins of a spectrum into the bands a display draws.
//!
//! # Layout
//!
//! - [`layout`] places parts along a panel's width or height.

/// A place or a size, across and down.
pub type Point = [f32; 2];

pub mod axis;
pub mod fit;
pub mod flat;
pub mod furniture;
pub mod interaction;
pub mod layout;
pub mod response;
pub mod sent;
pub mod spectrum;
pub mod stroke;
