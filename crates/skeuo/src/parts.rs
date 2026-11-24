//! shapes of common panel hardware, sized in panel points and centered on the z axis

mod button;
mod fader;
mod grip;
mod knob;
mod led;
pub(crate) mod legend;
mod meter;
mod outline;
mod panel;
mod scale;
mod switch;

pub use button::{Button, button_transform};
pub use fader::{Cap, Fader, fader_slot, fader_transform};
pub use grip::Grip;
pub use knob::{Edge, KNOB_SWEEP, Knob, Top, knob_transform};
pub use led::{led, led_material, led_seat};
pub use legend::{legend, legend_gap, legend_width, lettered};
pub use meter::Meter;
pub use panel::{frame, plate, screw, screw_seat, slab};
pub use scale::{dashes, fader_ticks, ticks};
pub use switch::{Push, PushSwitch};

/// faces that turn from each other by less than about 35 degrees shade as one curve
const CREASE: f32 = 0.82;
/// the rings a shaped top is cut into from its rim to its middle
const TOP_RINGS: usize = 14;
/// the rings the roll from the side to the top is cut into
const ROLL: usize = 6;
/// how far paint and inlays stand off the surface they are on
const PAINT: f32 = 0.25;
