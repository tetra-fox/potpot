//! Pages of potpot's parts, one for each kind, under tabs.
//!
//! A strip of tabs switches the pages. `just example gallery` opens them in a window, and
//! `just render gallery out.png knobs 2` draws one page to a PNG file in place of the window.
//! The name after the file's name picks the page:
//!
//! - `knobs`: every knob style
//! - `faders`: every fader cap
//! - `buttons`: every button and push switch, out, on and held
//! - `lights`: small lights, lines of light, and lamps beside what shows their light
//! - `rings`: rings, arcs and stops of light around knobs
//! - `meters`: ladders of lights and a needle meter
//! - `pads`: XY pads
//! - `screens`: screens that show an equalizer, a scope and an envelope
//! - `retro`: a whole soft synth's front in an old manner
//! - `industrial`: dark metal hardware lit from within
//! - `materials`: every material, on a knob and two strips
//!
//! After the page's name, a number is the scale the picture is drawn at. After it, one number
//! is the pixel size and a second the color steps, for the crude looks. `--first` anywhere
//! draws the first sample alone.

mod panels;

use potpot::{Look, Renderer, Scene};
use potpot_window::controls::flat::Flat;
use potpot_window::controls::interaction::Target;
use potpot_window::{Gpu, Opening, Settings, Skin, View, wgpu};

use crate::panels::shared;

/// The pages, drawn by potpot's renderer.
struct Gallery {
  renderer: Renderer,
  look: Look,
  /// Each page's panel and the target of each of its objects.
  ///
  /// A page's panel is made the first time it is shown.
  panels: Vec<Option<(Scene, Vec<Option<Target>>)>>,
  /// The page last laid out.
  shown: usize,
}

impl Gallery {
  fn new(gpu: &Gpu, look: Look) -> Self {
    Self {
      renderer: Renderer::new(gpu.device, gpu.format),
      look,
      panels: vec![None; panels::pages()],
      shown: 0,
    }
  }
}

impl Skin for Gallery {
  fn prepare(&mut self, gpu: &Gpu, view: &View, _: &mut Flat) {
    let look = self.look;
    let (scene, _) = self.panels[view.page].get_or_insert_with(|| panels::panel(view.page, look));
    (self.renderer).prepare(gpu.device, gpu.queue, scene, view.pixels);
    self.shown = view.page;
  }

  fn draw(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView, place: [u32; 4]) {
    shared::render(&self.renderer, encoder, target, place);
  }

  fn changed(&self) -> Option<&[[u32; 4]]> {
    self.renderer.changed()
  }

  fn settled(&self) -> bool {
    self.renderer.settled()
  }

  fn target_at(&self, at: [f32; 2]) -> Option<Target> {
    let object = self.renderer.pick(at.into())?;
    let (_, targets) = self.panels[self.shown]
      .as_ref()
      .expect("the page shown was laid out");
    targets[object]
  }
}

/// Ends the process with what is wrong with the arguments.
///
/// It says how the gallery is run as well.
fn wrong(what: &str) -> ! {
  eprintln!(
    "{what}\nusage: just render gallery <file.png> <page> [scale] [pixel size] [color steps]"
  );
  std::process::exit(2)
}

/// Returns the number an argument gives, if it is given.
///
/// An argument that is no such number ends the process, with `what` it was to be.
fn parsed<T: std::str::FromStr>(given: Option<String>, what: &str) -> Option<T> {
  given.map(|given| {
    (given.parse()).unwrap_or_else(|_| wrong(&format!("the {what} is not a number: {given}")))
  })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut settings = Settings::new("potpot gallery", panels::SIZE);
  settings.pages = panels::pages();
  let first = std::env::args().any(|argument| argument == shared::window::FIRST);
  let mut arguments = std::env::args()
    .skip(1)
    .filter(|argument| argument != shared::window::FIRST);
  let Some(file) = arguments.next() else {
    let skin = |gpu: &Gpu| Gallery::new(gpu, Look::default());
    let opened = potpot_window::open(settings, Vec::new(), Vec::new(), Opening::alone(), skin)?;
    opened.window.run_until_closed()?;
    return Ok(());
  };
  let names = panels::names();
  let name = (arguments.next()).unwrap_or_else(|| wrong(&format!("no page: one of {names}")));
  settings.page = (panels::page(&name))
    .unwrap_or_else(|| wrong(&format!("no page called {name}: one of {names}")));
  let scale = parsed(arguments.next(), "scale").unwrap_or(1.0);
  let look = Look {
    pixel_size: parsed(arguments.next(), "pixel size").unwrap_or(1.0),
    color_steps: parsed(arguments.next(), "color steps").unwrap_or(0),
    ..Look::default()
  };
  if let Some(more) = arguments.next() {
    wrong(&format!("an argument follows the color steps: {more}"));
  }
  let drawn = potpot_window::picture(&settings, Vec::new(), Vec::new(), scale, |gpu| {
    Gallery::new(gpu, look)
  })?;
  let pixels = if first { &drawn.first } else { &drawn.pixels };
  shared::window::write_png(file.as_ref(), drawn.size, pixels)?;
  eprintln!("wrote {file}");
  Ok(())
}
