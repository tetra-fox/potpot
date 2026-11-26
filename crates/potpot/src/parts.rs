//! Shapes of common panel hardware, sized in panel points.
//!
//! Each shape is centered on the z axis unless its doc places it elsewhere. Every size is more than
//! zero.

mod button;
mod fader;
mod grip;
mod keybed;
mod knob;
mod led;
mod legend;
mod meter;
mod outline;
mod pad;
mod panel;
mod ring;
mod scale;
mod screen;
mod switch;

pub use button::Button;
pub use fader::{Cap, Fader, fader_slot, fader_transform};
pub use grip::Grip;
pub use keybed::Keybed;
pub use knob::{Edge, KNOB_SWEEP, Knob, Top, knob_transform};
pub use led::{led, led_lamp, led_material, led_seat};
pub use legend::legend;
pub use meter::Meter;
pub use pad::Pad;
pub use panel::{bezel, frame, plate, screw, screw_cross, screw_head, screw_seat, slab, slot};
pub use ring::LedRing;
pub use scale::{dashes, fader_ticks, ticks};
pub use screen::{Screen, picture};
pub use switch::PushSwitch;

/// The cosine of the turn under which faces shade as one curve.
///
/// The turn is about 35 degrees.
const CREASE: f32 = 0.82;
/// The count of rings from a shaped top's rim to its middle.
const TOP_RINGS: usize = 14;
/// The count of rings in the roll from the side to the top.
const ROLL: usize = 6;
/// The distance paint and inlays stand off their surface.
const PAINT: f32 = 0.25;
