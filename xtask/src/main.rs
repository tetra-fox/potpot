//! The workspace's build tasks, run as `cargo xtask <task>`.
//!
//! - `env` bakes the studio the panel is lit by into `crates/potpot/assets/`

mod environment;
mod hdr;

use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
  let root = Path::new(env!("CARGO_MANIFEST_DIR"))
    .parent()
    .expect("the xtask is inside the workspace");
  let task = std::env::args().nth(1);
  let done = match task.as_deref() {
    Some("env") => environment::bake(root),
    _ => {
      eprintln!("usage: cargo xtask env");
      return ExitCode::FAILURE;
    }
  };
  match done {
    Ok(()) => ExitCode::SUCCESS,
    Err(error) => {
      eprintln!("xtask: {error}");
      ExitCode::FAILURE
    }
  }
}
