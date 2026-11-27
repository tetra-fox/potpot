//! The gallery's pages, each a scene of potpot's parts.
//!
//! The gallery shows them in a window or draws one to a file, and the timing example turns a
//! knob of one.

#![allow(dead_code, reason = "the timing example draws one page of these")]

mod buttons;
mod faders;
mod industrial;
pub mod knobs;
mod lights;
mod materials;
mod meters;
mod pads;
mod retro;
mod rings;
mod screens;
mod selectors;
#[path = "../../shared/mod.rs"]
pub mod shared;

use std::sync::Arc;

use potpot::glam::{Mat4, Vec2, Vec3};
use potpot::parts::{self, Tabs};
use potpot::{Lamp, Light, Look, Material, Mesh, Object, Scene};
use potpot_controls::interaction::Target;

use self::shared::{ALUMINUM, BLACK, SEAT, key, key_ink, lettering};

/// The width and height of a page, under the tabs.
const PAGE: Vec2 = Vec2::new(1024.0, 640.0);
/// The height of the strip of tabs over the pages.
const TABS: f32 = 64.0;
/// The width and height of the tabs and a page together.
pub const SIZE: [f32; 2] = [PAGE.x, PAGE.y + TABS];

/// A function that makes a page's scene under a look.
type Page = fn(Look) -> Scene;

/// Each page's name and the function that makes it.
///
/// The name is the one the page is asked for by and printed on its tab.
const PAGES: [(&str, Page); 12] = [
  ("knobs", knobs::knobs),
  ("faders", faders::faders),
  ("buttons", buttons::buttons),
  ("lights", lights::lights),
  ("rings", rings::rings),
  ("meters", meters::meters),
  ("pads", pads::pads),
  ("screens", screens::screens),
  ("selectors", selectors::selectors),
  ("retro", retro::retro),
  ("industrial", industrial::industrial),
  ("materials", materials::materials),
];

/// Returns the names of the pages, as a message lists them.
pub fn names() -> String {
  PAGES.map(|(name, _)| name).join(", ")
}

/// Returns the place of the page of a name.
pub fn page(name: &str) -> Option<usize> {
  PAGES.iter().position(|(page, _)| *page == name)
}

/// Returns the count of pages.
pub fn pages() -> usize {
  PAGES.len()
}

/// Returns a transform on a page as a transform on the panel.
pub fn on_panel(on_page: Mat4) -> Mat4 {
  Mat4::from_translation(Vec3::new(0.0, TABS, 0.0)) * on_page
}

/// Returns the panel showing a page, and its objects' targets.
///
/// The page is under a strip with a tab for every page, whose own tab is in and lit. A press on
/// a tab is on its page, and a press on the page itself is on nothing.
pub fn panel(page: usize, look: Look) -> (Scene, Vec<Option<Target>>) {
  let mut scene = (PAGES[page].1)(look);
  for object in &mut scene.objects {
    object.transform = on_panel(object.transform);
  }
  for lamp in &mut scene.lamps {
    lamp.at.y += TABS;
  }
  let mut targets = vec![None; scene.objects.len()];

  let strip = Object {
    mesh: Arc::new(parts::plate(PAGE.x, TABS, 6.0)),
    transform: Mat4::IDENTITY,
    material: Material::anodized(Vec3::new(0.2, 0.21, 0.23)),
  };
  scene.objects.push(strip);
  targets.push(None);
  let tabs = Tabs::new(PAGES.len(), 78.0, 30.0, 6.0);
  let (body, seat) = (Arc::new(tabs.body()), Arc::new(tabs.seat(1.5)));
  let middle = Vec2::new(PAGE.x / 2.0, TABS / 2.0);
  for (tab, (name, _)) in PAGES.into_iter().enumerate() {
    let shown = tab == page;
    let at = middle + tabs.middle(tab);
    let name = lettering(&name.to_uppercase(), KEY_LETTERS);
    scene.objects.extend([
      Object {
        mesh: Arc::clone(&seat),
        transform: Mat4::from_translation(at.extend(0.0)),
        material: Material::matte(SEAT),
      },
      Object {
        mesh: Arc::clone(&body),
        transform: tabs.transform(middle.x, middle.y, tab, shown),
        material: key(shown),
      },
      Object {
        mesh: Arc::new(name),
        transform: tabs.name(middle.x, middle.y, tab, shown),
        material: key_ink(),
      },
    ]);
    targets.extend([None, Some(Target::Page(tab)), Some(Target::Page(tab))]);
  }
  scene.size = Vec2::from(SIZE);
  (scene, targets)
}

/// Returns the transform of a place on a page.
fn at(place: Vec2) -> Mat4 {
  Mat4::from_translation(place.extend(0.0))
}

/// Returns a screw for each corner of a page.
///
/// Each is turned its own way.
fn screws() -> [Object; 4] {
  let screw = Arc::new(parts::screw(8.0));
  let inset = 18.0;
  [
    (inset, inset),
    (PAGE.x - inset, inset),
    (inset, PAGE.y - inset),
    (PAGE.x - inset, PAGE.y - inset),
  ]
  .map(|(x, y)| Object {
    mesh: Arc::clone(&screw),
    transform: Mat4::from_translation(Vec3::new(x, y, 0.0)) * Mat4::from_rotation_z(x * 0.3 + y),
    material: Material::chrome(Vec3::splat(0.75)),
  })
}

/// Returns the scene of a page that fills it.
///
/// The page is seen leaning a little, under the plain light and with no lamps.
fn framed(look: Look, objects: Vec<Object>) -> Scene {
  Scene {
    size: PAGE,
    heights: [-8.0, 64.0],
    lean: Vec2::new(0.0, -0.25),
    light: Light::default(),
    look,
    objects,
    lamps: Vec::new(),
  }
}

/// A page being put together.
///
/// It holds what stands where and what is lit.
#[derive(Default)]
struct Build {
  objects: Vec<Object>,
  lamps: Vec<Lamp>,
}

impl Build {
  /// Adds a mesh that is placed as often as wanted.
  fn place(&mut self, mesh: &Arc<Mesh>, transform: Mat4, material: Material) {
    self.objects.push(Object {
      mesh: Arc::clone(mesh),
      transform,
      material,
    });
  }

  /// Adds a mesh that goes in one place only.
  fn put(&mut self, mesh: Mesh, transform: Mat4, material: Material) {
    self.place(&Arc::new(mesh), transform, material);
  }

  /// Adds lettering centered on a point at a height off the panel.
  fn letters(&mut self, text: &str, at: Vec3, tall: f32, material: Material) {
    self.put(lettering(text, tall), Mat4::from_translation(at), material);
  }
}

/// The room between a catalog's grid and its page's sides.
const MARGIN: f32 = 32.0;
/// How far down its page a catalog's title is centered.
const TITLE: f32 = 40.0;
/// The top of a catalog's grid, down from the top of its page.
const GRID_TOP: f32 = 64.0;
/// The distance of a catalog's grid from the foot of its page.
const GRID_FOOT: f32 = 24.0;
/// The height of the band under a cell's specimen.
///
/// The band holds the cell's label.
const LABEL_BAND: f32 = 28.0;

/// One cell of a catalog's grid.
///
/// It holds one specimen and its label.
#[derive(Clone, Copy)]
struct Cell {
  /// The middle of the specimen's room, over the label.
  middle: Vec2,
  /// The width and height of that room.
  room: Vec2,
}

/// A page that shows the variations of one kind of part.
///
/// It is a plate with its title over a grid of cells, each a specimen with its label under it.
/// Every catalog has the same margins, spacing and lettering.
struct Catalog {
  build: Build,
  /// The ink the title and the labels are printed in.
  ink: Material,
  cells: Vec<Cell>,
}

/// Returns the light over a dark page, dimmed so that what glows shows.
fn dim() -> Light {
  Light {
    color: Vec3::splat(0.6),
    ..Light::default()
  }
}

/// The height of the lettering on a key.
const KEY_LETTERS: f32 = 6.5;

/// Returns the pale rubber of buttons and pucks.
fn button_rubber() -> Material {
  Material::silicone(Vec3::new(0.62, 0.62, 0.60))
}

/// Returns the cells of a grid of `columns` by `rows`.
///
/// The cells go row by row from the top left. Each cell's specimen has `room` points of height
/// over its label. The grid is as wide as the page less its margins, and stands in the middle
/// of the height under the title.
fn cells([columns, rows]: [usize; 2], room: f32) -> Vec<Cell> {
  let width = (PAGE.x - MARGIN * 2.0) / columns as f32;
  let size = Vec2::new(width, room + LABEL_BAND);
  let free = PAGE.y - GRID_TOP - GRID_FOOT - size.y * rows as f32;
  let top = GRID_TOP + free / 2.0;
  let room = Vec2::new(width, room);
  let cell = |index: usize| {
    let corner =
      Vec2::new(MARGIN, top) + size * Vec2::new((index % columns) as f32, (index / columns) as f32);
    Cell {
      middle: corner + room / 2.0,
      room,
    }
  };
  (0..rows * columns).map(cell).collect()
}

impl Catalog {
  /// Creates a catalog on a plate of `plate`.
  ///
  /// Its `grid` is in columns and rows, with `room` points of height for each specimen, as
  /// [`cells`] lays them out. The title and the labels are printed in `ink`.
  fn new(title: &str, plate: Material, ink: Material, grid: [usize; 2], room: f32) -> Self {
    let mut build = Build::default();
    build.put(parts::plate(PAGE.x, PAGE.y, 6.0), Mat4::IDENTITY, plate);
    build.letters(title, Vec3::new(PAGE.x / 2.0, TITLE, 0.0), 14.0, ink);
    Self {
      build,
      ink,
      cells: cells(grid, room),
    }
  }

  /// Creates a catalog on a dark plate, with its lettering pale.
  ///
  /// The grid and the room are as [`Catalog::new`] takes them.
  fn dark(title: &str, grid: [usize; 2], room: f32) -> Self {
    let plate = Material::powder_coat(Vec3::new(0.075, 0.08, 0.09));
    Self::new(title, plate, Material::ink(Vec3::splat(0.8)), grid, room)
  }

  /// Prints a cell's label in the band at its foot.
  fn label(&mut self, cell: Cell, text: &str) {
    let at = cell.middle + Vec2::new(0.0, (cell.room.y + LABEL_BAND) / 2.0);
    self.build.letters(text, at.extend(0.0), 9.0, self.ink);
  }

  /// Returns the scene of the finished page.
  ///
  /// A screw goes in each corner.
  fn finish(mut self, look: Look, light: Light) -> Scene {
    self.build.objects.extend(screws());
    Scene {
      light,
      lamps: self.build.lamps,
      ..framed(look, self.build.objects)
    }
  }
}
