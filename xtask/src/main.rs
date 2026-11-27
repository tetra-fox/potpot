//! The workspace's build tasks, run as `cargo xtask <task>`.
//!
//! - `assets` bakes everything the renderer embeds into `crates/potpot/assets/`
//! - `env` bakes only the studio the panel is lit by

mod environment;
mod hdr;
mod textures;

use std::path::Path;
use std::process::ExitCode;

/// The most bytes the renderer's embedded assets take together.
const BUDGET: u64 = 8 * 1024 * 1024;
/// Every file the renderer embeds, under the workspace.
const EMBEDDED: [&str; 5] = [
  environment::REST,
  environment::SOFTBOX,
  textures::ALBEDO,
  textures::NORMAL,
  textures::ROUGHNESS,
];

fn main() -> ExitCode {
  let root = Path::new(env!("CARGO_MANIFEST_DIR"))
    .parent()
    .expect("the xtask is inside the workspace");
  let task = std::env::args().nth(1);
  let baked = match task.as_deref() {
    Some("assets") => environment::bake(root).and_then(|()| textures::bake(root)),
    Some("env") => environment::bake(root),
    _ => {
      eprintln!("usage: cargo xtask assets | env");
      return ExitCode::FAILURE;
    }
  };
  match baked.and_then(|()| within_budget(root)) {
    Ok(()) => ExitCode::SUCCESS,
    Err(error) => {
      eprintln!("xtask: {error}");
      ExitCode::FAILURE
    }
  }
}

/// Returns an error if the embedded assets together are over [`BUDGET`].
///
/// A file not baked yet counts for nothing.
fn within_budget(root: &Path) -> Result<(), String> {
  let mut total = 0;
  for path in EMBEDDED {
    match std::fs::metadata(root.join(path)) {
      Ok(metadata) => total += metadata.len(),
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
      Err(error) => return Err(format!("{path}: {error}")),
    }
  }
  if total > BUDGET {
    return Err(format!(
      "the embedded assets take {total} bytes, over the budget of {BUDGET}"
    ));
  }
  Ok(())
}

/// Writes Rust source to a file and formats it as the workspace does.
///
/// # Errors
///
/// Returns an error if the file cannot be written or `rustfmt` fails on it.
fn write_source(path: &Path, source: &str) -> Result<(), String> {
  std::fs::write(path, source).map_err(|error| format!("{}: {error}", path.display()))?;
  let formatted = std::process::Command::new("rustfmt")
    .args(["--edition", "2024"])
    .arg(path)
    .status()
    .map_err(|error| format!("rustfmt could not run: {error}"))?;
  if !formatted.success() {
    return Err(format!("rustfmt failed on {}", path.display()));
  }
  Ok(())
}
