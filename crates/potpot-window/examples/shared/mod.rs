//! The two ways an example is run: in a window, or drawn to a PNG file.
//!
//! An example includes this with `mod shared;`. The examples of `potpot` reach it through their
//! own shared module.

#![allow(dead_code, reason = "each example uses a part of this")]

use std::error::Error;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use potpot_window::controls::interaction::Control;
use potpot_window::{Gpu, Opening, Param, Settings, Skin};

/// A stand-in for a plugin's audio thread.
pub struct Sound {
  /// The function that runs one block of it.
  pub block: Box<dyn FnMut() + Send>,
  /// The time between two blocks while the window is open.
  pub every: Duration,
  /// The count of blocks that are run before a picture is drawn.
  pub pictured_after: u32,
}

/// Returns the PNG file the example is asked to draw itself to.
///
/// It is the example's first argument, and `None` if the example is to open its window.
pub fn file() -> Option<PathBuf> {
  std::env::args_os().nth(1).map(PathBuf::from)
}

/// Opens an example's window or draws the example to a file.
///
/// The file is the one [`file`] returns, and with none the window opens. In a window, `sound`
/// runs on a thread of its own, a block at a time. Before a picture its blocks are run one after
/// another with no time between them, so the same run gives the same picture. The picture is
/// drawn at the scale the example's second argument gives, or at the panel's own size.
///
/// A second argument that is no number, or a third, ends the process with what is wrong and how
/// the example is run.
pub fn run<S: Skin>(
  settings: Settings,
  params: Vec<Param>,
  controls: Vec<Control>,
  sound: Option<Sound>,
  skin: impl FnOnce(&Gpu) -> S + Send + 'static,
) -> Result<(), Box<dyn Error>> {
  let Some(file) = file() else {
    if let Some(mut sound) = sound {
      // the thread is left running, and ends with the process when the window closes
      thread::spawn(move || {
        loop {
          (sound.block)();
          // an audio thread is called back for each block, which this stands in for
          thread::sleep(sound.every);
        }
      });
    }
    let opened = potpot_window::open(settings, params, controls, Opening::alone(), skin)?;
    opened.window.run_until_closed()?;
    return Ok(());
  };
  let scale = scale().unwrap_or_else(|wrong| {
    eprintln!("{wrong}\nusage: just render <example> <file.png> [scale]");
    std::process::exit(2)
  });
  if let Some(mut sound) = sound {
    for _ in 0..sound.pictured_after {
      (sound.block)();
    }
  }
  let drawn = potpot_window::picture(&settings, params, controls, scale, skin)?;
  write_png(&file, drawn.size, &drawn.pixels)?;
  eprintln!("wrote {}", file.display());
  Ok(())
}

/// Returns the scale a picture is asked for, which is one if none is.
///
/// It is the example's second argument.
///
/// # Errors
///
/// Returns what is wrong if the argument is no number or another follows it.
fn scale() -> Result<f32, String> {
  let mut after_file = std::env::args_os().skip(2);
  let scale = match after_file.next() {
    Some(given) => {
      let given = given.to_string_lossy();
      let unread = |_| format!("the scale after the file is not a number: {given}");
      given.parse().map_err(unread)?
    }
    None => 1.0,
  };
  match after_file.next() {
    Some(more) => Err(format!(
      "an argument follows the scale: {}",
      more.to_string_lossy()
    )),
    None => Ok(scale),
  }
}

/// Writes a picture to a PNG file.
///
/// `pixels` holds the picture's rows from the top, four bytes to a pixel: red, green, blue and
/// one more, which is left out of the file.
pub fn write_png(file: &Path, size: [u32; 2], pixels: &[u8]) -> Result<(), Box<dyn Error>> {
  let mut png = png::Encoder::new(BufWriter::new(File::create(file)?), size[0], size[1]);
  png.set_color(png::ColorType::Rgb);
  let colors: Vec<u8> = (pixels.as_chunks::<4>().0.iter())
    .flat_map(|&[red, green, blue, _]| [red, green, blue])
    .collect();
  png.write_header()?.write_image_data(&colors)?;
  Ok(())
}
