//! Panel furniture, readouts and screen pictures that the examples share.
//!
//! An example includes this with `mod shared;` and keeps what is its own: its parameters, its
//! controls and where everything stands.

#![allow(dead_code, reason = "each example uses a part of this")]

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use potpot::glam::{Mat4, Vec2, Vec3};
use potpot::parts;
use potpot::{Material, Mesh, Object, Place, Renderer};
use potpot_window::controls::flat::{Color, Flat};
use potpot_window::controls::interaction::{Dial, Target};
use potpot_window::controls::layout::{self, Pack};
use potpot_window::{Param, View, wgpu};

// the way an example is run, which the one of potpot-window shares
#[path = "../../../potpot-window/examples/shared/mod.rs"]
pub mod window;

/// The black of knob bodies, caps and the frames of readouts.
pub const BLACK: Vec3 = Vec3::new(0.035, 0.035, 0.04);
/// The color of aluminum turned on a lathe.
pub const ALUMINUM: Vec3 = Vec3::new(0.78, 0.79, 0.81);
/// The color of the dark gap around what is set into a panel.
pub const SEAT: Vec3 = Vec3::splat(0.012);
/// The color of a screen's face.
const FACE: Vec3 = Vec3::new(0.012, 0.016, 0.018);
/// The color of the lines that divide a screen.
pub const GRID: Color = [0.09, 0.13, 0.15, 1.0];
/// The color of the lettering on a screen that names its lines.
pub const LABELS: Color = [0.3, 0.42, 0.46, 1.0];
/// The color of a response curve on a screen.
pub const CYAN: Color = [0.45, 0.95, 1.0, 1.0];
/// The amber a readout's value is written in.
pub const INK: Color = [1.0, 0.66, 0.14, 1.0];
/// The color of the block under selected letters.
const SELECTED: Color = [0.45, 0.28, 0.05, 1.0];

/// Returns lettering `tall` high.
///
/// Its strokes are wide enough to show when it is small.
pub fn lettering(text: &str, tall: f32) -> Mesh {
  parts::legend(text, tall, (tall * 0.17).max(1.0))
}

/// Returns the middles of `N` slots centered in a span.
///
/// The slots are each `slot` wide and touch.
pub fn centered<const N: usize>(span: [f32; 2], slot: f32) -> [f32; N] {
  let slots = layout::pack(span, &[slot; N], Pack::Center);
  std::array::from_fn(|place| layout::middle(slots[place]))
}

/// Objects on their way into a scene.
#[derive(Clone, Default)]
pub struct Drawn {
  pub objects: Vec<Object>,
  /// What a press on each of the objects is on.
  pub targets: Vec<Option<Target>>,
}

impl Drawn {
  /// Adds a mesh that is placed as often as wanted.
  pub fn place(
    &mut self,
    mesh: &Arc<Mesh>,
    transform: Mat4,
    material: Material,
    target: Option<Target>,
  ) {
    self.objects.push(Object {
      mesh: Arc::clone(mesh),
      transform,
      material,
    });
    self.targets.push(target);
  }

  /// Adds a mesh that goes in one place only.
  ///
  /// No press is on it.
  pub fn put(&mut self, mesh: Mesh, at: Vec2, material: Material) {
    let transform = Mat4::from_translation(at.extend(0.0));
    self.place(&Arc::new(mesh), transform, material, None);
  }

  /// Adds lettering centered on a point.
  pub fn letters(&mut self, text: &str, at: Vec2, tall: f32, material: Material) {
    self.put(lettering(text, tall), at, material);
  }

  /// Adds objects a part made itself, which no press is on.
  pub fn extend(&mut self, objects: impl IntoIterator<Item = Object>) {
    self.objects.extend(objects);
    self.targets.resize(self.objects.len(), None);
  }

  /// Adds a knob's body and its painted line, turned to a value.
  ///
  /// `turn` is a part of the whole turn of [`parts::KNOB_SWEEP`], and a press on either is on
  /// `control`.
  pub fn knob(
    &mut self,
    meshes: &[Arc<Mesh>; 2],
    materials: [Material; 2],
    at: Vec2,
    turn: f32,
    control: usize,
  ) {
    let transform = parts::knob_transform(at.x, at.y, turn, parts::KNOB_SWEEP);
    for (mesh, material) in meshes.iter().zip(materials) {
      self.place(mesh, transform, material, Some(Target::Control(control)));
    }
  }

  /// Adds a fader's black cap and its white stripe.
  ///
  /// The cap is at `value` of the way along its travel.
  ///
  /// The track is `travel` long with its middle at `at`, and a press on the cap is on
  /// `control`.
  pub fn fader(
    &mut self,
    meshes: &[Arc<Mesh>; 2],
    at: Vec2,
    travel: f32,
    value: f32,
    control: usize,
  ) {
    let transform = parts::fader_transform(at.x, at.y, travel, value);
    for (mesh, material) in meshes.iter().zip(cap_materials()) {
      self.place(mesh, transform, material, Some(Target::Control(control)));
    }
  }

  /// Adds a screw in a dark seat at each corner of a panel.
  ///
  /// Each is turned its own way.
  ///
  /// The panel is `size` across and down, and the screws are `inset` from its edges.
  pub fn screws(&mut self, size: Vec2, inset: f32, radius: f32) {
    let seat = Arc::new(parts::screw_seat(radius, 1.5));
    let screw = Arc::new(parts::screw(radius));
    for corner in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)].map(Vec2::from) {
      let at = Vec2::splat(inset) + corner * (size - Vec2::splat(inset * 2.0));
      let on_panel = Mat4::from_translation(at.extend(0.0));
      self.place(&seat, on_panel, Material::matte(SEAT), None);
      let turned = on_panel * Mat4::from_rotation_z(at.x * 0.3 + at.y);
      self.place(&screw, turned, Material::chrome(Vec3::splat(0.8)), None);
    }
  }

  /// Adds the window a control's value is written in.
  ///
  /// The window is a frame around dark glass.
  ///
  /// The glass is `half` to each side of `middle`. A press on it is on the readout of
  /// `control`, and [`written`] writes the value over it.
  pub fn readout(&mut self, middle: Vec2, half: Vec2, control: usize) {
    let (wide, tall) = (half.x * 2.0, half.y * 2.0);
    let target = Some(Target::Readout(control));
    let on_panel = Mat4::from_translation(middle.extend(0.0));
    let frame = parts::frame(wide + 6.0, tall + 6.0, 3.0, 2.0, 3.0);
    self.place(&Arc::new(frame), on_panel, Material::glossy(BLACK), target);
    let glass = Mesh::cuboid(
      Vec3::new(-half.x, -half.y, 0.0),
      Vec3::new(half.x, half.y, 0.4),
    );
    let pane = Material::pane(Vec3::splat(0.01));
    self.place(&Arc::new(glass), on_panel, pane, target);
  }
}

/// Returns the materials of a fader's cap and of its stripe.
///
/// The cap is black and the stripe white.
pub fn cap_materials() -> [Material; 2] {
  [
    Material::glossy(Vec3::splat(0.04)),
    Material::matte(Vec3::splat(0.9)),
  ]
}

/// Adds a control's value, or the value being typed into it.
///
/// Its middle is at `middle`.
///
/// The letters are `tall` high and go flat over the glass that [`Drawn::readout`] makes.
pub fn written(over: &mut Flat, view: &View, control: usize, middle: Vec2, tall: f32) {
  let weight = (tall * 0.15).max(1.0);
  match view.edit.filter(|edit| edit.control == control) {
    Some(edit) => over.typed(edit, middle.into(), tall, weight, INK, SELECTED),
    None => {
      let reading = &view.controls[control];
      let value = format!("{}{}", reading.text, reading.unit);
      over.centered(&value, middle.into(), tall, weight, INK);
    }
  }
}

/// The part of a screen its picture is drawn in.
#[derive(Clone, Copy)]
pub struct Plot {
  /// The top left corner, in the panel's points.
  pub min: Vec2,
  /// The bottom right corner.
  pub max: Vec2,
}

impl Plot {
  /// Returns the plot of a screen at `middle`.
  ///
  /// It is `inset` from the edges of the screen's face.
  pub fn of(screen: &parts::Screen, middle: Vec2, inset: f32) -> Self {
    let half = Vec2::new(screen.width, screen.length) / 2.0 - Vec2::splat(inset);
    Self {
      min: middle - half,
      max: middle + half,
    }
  }

  /// Returns the plot's width and height.
  ///
  /// A dot dragged over the whole plot has this as the room of its pad.
  pub fn size(&self) -> Vec2 {
    self.max - self.min
  }

  /// Returns the point of the panel at a place in the plot.
  ///
  /// The place is a part of the way across the plot and a part of the way up it, which is where
  /// two parameters stand whose axes the plot is drawn along.
  pub fn at(&self, [across, up]: [f32; 2]) -> [f32; 2] {
    let size = self.size();
    [self.min.x + across * size.x, self.max.y - up * size.y]
  }

  /// Adds lines that divide the plot into columns and rows.
  ///
  /// There are `across` columns and `up` rows.
  pub fn divide(&self, flat: &mut Flat, across: u8, up: u8) {
    for column in 0..=across {
      let x = f32::from(column) / f32::from(across);
      flat.line(self.at([x, 0.0]), self.at([x, 1.0]), 1.0, GRID);
    }
    for row in 0..=up {
      let y = f32::from(row) / f32::from(up);
      flat.line(self.at([0.0, y]), self.at([1.0, y]), 1.0, GRID);
    }
  }
}

/// Adds a dot for something on a screen that is dragged.
///
/// The dot is a ring of `color` `reach` out from `at`, and filled while `held`.
pub fn dot(flat: &mut Flat, at: [f32; 2], reach: f32, color: Color, held: bool) {
  flat.line(at, at, reach * 2.0, color);
  if !held {
    let [red, green, blue, cover] = color;
    let dark = [red * 0.1, green * 0.1, blue * 0.1, cover];
    flat.line(at, at, reach, dark);
  }
}

/// Returns the last of `dots` that a point is on.
///
/// Each dot reaches `reach` out from its middle, and the one drawn last lies over the others.
pub fn dot_at(dots: &[[f32; 2]], at: [f32; 2], reach: f32) -> Option<usize> {
  dots
    .iter()
    .rposition(|dot| (at[0] - dot[0]).hypot(at[1] - dot[1]) <= reach)
}

/// A picture on a screen, with the objects that show it.
///
/// It keeps the source it was drawn from, and is drawn and its objects made again when it is
/// shown from a source that differs. So a frame with the same pictures costs the renderer no
/// drawing, and one where a single picture is new makes that picture's objects alone.
pub struct Picture<Source> {
  source: Option<Source>,
  pub objects: Vec<Object>,
}

impl<Source: PartialEq> Picture<Source> {
  pub fn new() -> Self {
    Self {
      source: None,
      objects: Vec::new(),
    }
  }

  /// Shows the picture that `flat` draws from `source`.
  ///
  /// The picture is on the screen at `middle`, whose face is of `glass`, over `under` objects
  /// already on that screen.
  pub fn show(
    &mut self,
    source: Source,
    flat: impl FnOnce(&Source) -> Flat,
    middle: Vec2,
    glass: Material,
    under: usize,
  ) {
    if self.source.as_ref() != Some(&source) {
      self.objects = parts::picture(&flat(&source), middle.x, middle.y, glass, under);
      self.source = Some(source);
    }
  }
}

/// Returns the material of a screen's face.
pub fn glass() -> Material {
  Material::screen(FACE)
}

/// Returns a parameter for each dial, and where each stands.
///
/// No host has these parameters: each stands where the panel last set it. The values are
/// shared, so that a thread standing in for the audio thread reads them with [`value`], the way
/// a plugin's audio thread reads its parameters.
pub fn params(dials: Vec<Dial>) -> (Vec<Param>, Arc<[AtomicU32]>) {
  let values: Arc<[AtomicU32]> = (dials.iter())
    .map(|dial| AtomicU32::new(dial.default.to_bits()))
    .collect();
  let params = (dials.into_iter().enumerate())
    .map(|(parameter, dial)| {
      let (read, write) = (Arc::clone(&values), Arc::clone(&values));
      Param::new(
        dial,
        move || value(&read, parameter),
        || {},
        move |to| set(&write, parameter, to),
        || {},
      )
    })
    .collect();
  (params, values)
}

/// Returns where a parameter stands, as a part of its turn.
pub fn value(values: &[AtomicU32], parameter: usize) -> f32 {
  f32::from_bits(values[parameter].load(Ordering::Relaxed))
}

/// Puts a parameter somewhere, as a part of its turn.
pub fn set(values: &[AtomicU32], parameter: usize, to: f32) {
  values[parameter].store(to.to_bits(), Ordering::Relaxed);
}

/// Draws what a renderer last prepared onto a target.
///
/// `place` is as [`Skin::draw`](potpot_window::Skin::draw) is given it.
pub fn render(
  renderer: &Renderer,
  encoder: &mut wgpu::CommandEncoder,
  target: &wgpu::TextureView,
  place: [u32; 4],
) {
  let [x, y, width, height] = place.map(|side| side as f32);
  let on = Place {
    x,
    y,
    width,
    height,
  };
  renderer.render(encoder, target, on, place);
}

/// A made-up noise, the same on every run.
pub struct Noise(u32);

impl Noise {
  pub fn new() -> Self {
    Self(0x2545_f491)
  }

  /// Returns the next number, from minus a half to a half.
  pub fn sample(&mut self) -> f32 {
    // a xorshift step, whose top bits are spread over the range
    self.0 ^= self.0 << 13;
    self.0 ^= self.0 >> 17;
    self.0 ^= self.0 << 5;
    (self.0 >> 8) as f32 / (1 << 24) as f32 - 0.5
  }
}
