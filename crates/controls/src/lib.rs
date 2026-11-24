//! what the controls of an audio plugin's panel do, apart from how they look, what window
//! they are in and what plugin they belong to. nothing here draws, and nothing here depends
//! on anything: a panel's look is built on top, with whatever it is drawn by

#![warn(missing_docs, missing_debug_implementations)]

pub mod interaction;
pub mod sent;
pub mod stroke;
