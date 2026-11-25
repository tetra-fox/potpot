//! lit buttons, push switches and level meters

use std::f32::consts::FRAC_PI_2;
use std::sync::Arc;

use potpot::glam::{Mat4, Vec2, Vec3};
use potpot::parts::{self, Button, Meter, Push, PushSwitch};
use potpot::{Light, Look, Material, Object, Scene};

use crate::SIZE;

/// rubber buttons, some of them lit, and level meters at a few levels, on a dark panel
/// under a dimmed light so that what glows shows
pub(super) fn lights(look: Look) -> Scene {
  let (width, height) = (SIZE[0] as f32, SIZE[1] as f32);
  let rubber = Material::silicone(Vec3::new(0.62, 0.62, 0.60));
  let colors = [
    Vec3::new(1.0, 0.25, 0.05),
    Vec3::new(0.1, 0.9, 0.3),
    Vec3::new(0.1, 0.5, 1.0),
    Vec3::new(1.0, 0.75, 0.1),
    Vec3::new(0.9, 0.1, 0.7),
    Vec3::new(0.95, 0.95, 0.9),
  ];

  let pad = Button::pad(70.0, 12.0);
  let round = Button::round(46.0, 10.0);
  let key = Button::key(70.0, 28.0, 7.0);
  let pill = Button::pill(26.0, 60.0, 8.0);
  let wide = Meter::ladder(16, 18.0, 420.0);
  let thin = Meter::ladder(24, 9.0, 420.0);
  let lying = Meter::ladder(14, 20.0, 250.0);
  // a shape's body and then its seat, or for a switch its plunger and then its collar
  let pair = |body: potpot::Mesh, seat: potpot::Mesh| [Arc::new(body), Arc::new(seat)];
  let pads = pair(pad.body(), pad.seat(3.0));
  let rounds = pair(round.body(), round.seat(2.5));
  let keys = pair(key.body(), key.seat(2.5));
  let pills = pair(pill.body(), pill.seat(2.5));
  let wides = pair(wide.segment(), wide.seat());
  let thins = pair(thin.segment(), thin.seat());
  let lyings = pair(lying.segment(), lying.seat());
  let square = PushSwitch::square(30.0, 18.0);
  let squares = pair(square.plunger(), square.collar());
  let circle = PushSwitch::round(30.0, 18.0);
  let circles = pair(circle.plunger(), circle.collar());
  let screw = Arc::new(parts::screw(8.0));
  let mut objects = vec![Object {
    mesh: Arc::new(parts::plate(width, height, 6.0)),
    transform: Mat4::IDENTITY,
    material: Material::textured(Vec3::new(0.075, 0.08, 0.09)),
  }];
  let mut lamps = Vec::new();
  let seat = Material::matte(Vec3::splat(0.012));

  // each button with the meshes of its body and its seat, where it is, how far it is lit
  // and in which color, and how far it is held down
  let mut button = |shape: Button,
                    [body, gap]: &[Arc<potpot::Mesh>; 2],
                    x: f32,
                    y: f32,
                    level: f32,
                    color: Vec3,
                    held: f32| {
    objects.push(Object {
      mesh: Arc::clone(gap),
      transform: Mat4::from_translation(Vec3::new(x, y, 0.0)),
      material: seat,
    });
    objects.push(Object {
      mesh: Arc::clone(body),
      transform: parts::button_transform(x, y, shape.height, held),
      material: rubber.lit(color, level, shape.glow_radius()),
    });
    if level > 0.0 {
      lamps.push(shape.lamp(x, y, color, level));
    }
  };
  // a grid of pads, lit here and there, one of them dimly and one held down
  for index in 0..16 {
    let (x, y) = (
      72.0 + 86.0 * (index % 4) as f32,
      82.0 + 86.0 * (index / 4) as f32,
    );
    let level = match index {
      0 | 5 | 6 | 11 | 13 => 1.0,
      9 => 0.35,
      _ => 0.0,
    };
    let held = if index == 6 { 1.0 } else { 0.0 };
    button(pad, &pads, x, y, level, colors[index % colors.len()], held);
  }
  for index in 0..5 {
    let level = if index == 1 || index == 4 { 1.0 } else { 0.0 };
    button(
      round,
      &rounds,
      72.0 + 64.5 * index as f32,
      470.0,
      level,
      colors[index + 1],
      0.0,
    );
  }
  for index in 0..4 {
    let level = if index == 2 { 1.0 } else { 0.0 };
    button(
      key,
      &keys,
      72.0 + 86.0 * index as f32,
      560.0,
      level,
      colors[3],
      0.0,
    );
  }
  for index in 0..12 {
    let (x, y) = (
      440.0 + 44.0 * (index % 6) as f32,
      110.0 + 84.0 * (index / 6) as f32,
    );
    let level = if index % 6 == index / 6 * 3 + 1 {
      1.0
    } else {
      0.0
    };
    button(
      pill,
      &pills,
      x,
      y,
      level,
      colors[(index + 2) % colors.len()],
      0.0,
    );
  }

  // push switches: out, latched in and lit, and held all the way in, square and then round
  let black = Material::glossy(Vec3::new(0.05, 0.05, 0.055));
  let collar = Material::chrome(Vec3::splat(0.7));
  let switches = [
    (square, Push::Latching, false, false),
    (square, Push::Latching, true, false),
    (square, Push::Momentary, true, true),
    (circle, Push::Latching, false, false),
    (circle, Push::Latching, true, false),
    (circle, Push::Momentary, true, true),
  ];
  for (index, (switch, push, on, held)) in switches.into_iter().enumerate() {
    let (x, y) = (452.0 + 48.0 * index as f32, 300.0);
    let round = index >= 3;
    let [plunger, ring] = if round { &circles } else { &squares };
    objects.push(Object {
      mesh: Arc::clone(ring),
      transform: Mat4::from_translation(Vec3::new(x, y, 0.0)),
      material: collar,
    });
    let amber = Vec3::new(1.0, 0.55, 0.05);
    let level = if on { 1.0 } else { 0.0 };
    objects.push(Object {
      mesh: Arc::clone(plunger),
      transform: switch.transform(x, y, push.depth(on, held)),
      material: if round {
        black
      } else {
        rubber.lit(amber, level, switch.plunger.glow_radius())
      },
    });
    if on && !round {
      lamps.push(switch.plunger.lamp(x, y, amber, 0.6));
    }
  }

  // three pairs of meters, each at its own level, the first pair holding a peak
  let foot = 540.0;
  let meters = [
    (wide, &wides, 760.0, 0.78, Some(0.92)),
    (wide, &wides, 792.0, 0.64, Some(0.80)),
    (thin, &thins, 870.0, 0.42, None),
    (thin, &thins, 888.0, 1.0, None),
    (thin, &thins, 940.0, 0.0, None),
    (thin, &thins, 958.0, 0.18, None),
  ];
  for (meter, [segment, gap], x, level, peak) in meters {
    objects.extend(meter.seats(gap, x, foot));
    objects.extend(meter.lights(segment, x, foot, level, peak));
    lamps.extend(meter.lamp(x, foot, level));
  }
  // and a row of lights lying on its side, which is a meter turned a quarter
  let across =
    Mat4::from_translation(Vec3::new(440.0, 420.0, 0.0)) * Mat4::from_rotation_z(FRAC_PI_2);
  let turned = |object: Object| Object {
    transform: across * object.transform,
    ..object
  };
  let [segment, gap] = &lyings;
  objects.extend(lying.seats(gap, 0.0, 0.0).map(turned));
  objects.extend(lying.lights(segment, 0.0, 0.0, 0.7, None).map(turned));

  let inset = 18.0;
  for (x, y) in [
    (inset, inset),
    (width - inset, inset),
    (inset, height - inset),
    (width - inset, height - inset),
  ] {
    objects.push(Object {
      mesh: Arc::clone(&screw),
      transform: Mat4::from_translation(Vec3::new(x, y, 0.0)) * Mat4::from_rotation_z(x * 0.3 + y),
      material: Material::chrome(Vec3::splat(0.75)),
    });
  }

  Scene {
    size: Vec2::new(width, height),
    heights: [-8.0, 64.0],
    lean: Vec2::new(0.0, -0.25),
    light: Light {
      color: Vec3::splat(0.6),
      ambient: Vec3::splat(0.2),
      ..Light::default()
    },
    look,
    objects,
    lamps,
  }
}
