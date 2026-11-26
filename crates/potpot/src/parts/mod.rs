//! Shapes of common panel hardware, sized in panel points.
//!
//! Each shape is centered on the z axis unless its doc places it elsewhere. Every size is more than
//! zero.

use glam::Vec3;

use crate::material::Material;

mod button;
mod fader;
mod grip;
mod handle;
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
mod vent;

pub use button::Button;
pub use fader::{Cap, Fader, fader_slot, fader_transform};
pub use grip::Grip;
pub use handle::Handle;
pub use keybed::Keybed;
pub use knob::{Edge, KNOB_SWEEP, Knob, Top, knob_transform};
pub use led::{led, led_lamp, led_material, led_seat, light_line, light_line_seat};
pub use legend::{legend, tracked_legend};
pub use meter::Meter;
pub use pad::Pad;
pub use panel::{
  bezel, cap_screw, chamfered_slab, frame, hex_socket, inset, plate, screw, screw_cross,
  screw_head, screw_seat, slab, slot,
};
pub use ring::{LedArc, LedRing, LedStops};
pub use scale::{dashes, fader_ticks, ticks};
pub use screen::{Screen, picture};
pub use switch::PushSwitch;
pub use vent::Vent;

/// Returns the material of the dark gap a light pipe's end is set in.
fn pipe_seat() -> Material {
  Material::matte(Vec3::splat(0.008))
}

/// Returns the material of a light pipe's end that shows `color` at `level`.
///
/// The end is smoky and barely tinted when dark, and when lit brightest over the light at the
/// bottom of the pipe and along its walls, fading toward `radius` away from its middle.
fn pipe_end(color: Vec3, level: f32, radius: f32) -> Material {
  let dark = Material::pipe();
  Material {
    color: dark.color + color * 0.03,
    ..dark
  }
  .lit(color, level, radius)
}

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
