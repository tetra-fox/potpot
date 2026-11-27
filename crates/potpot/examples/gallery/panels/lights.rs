//! Lit buttons, push switches, level meters and an XY pad.

use std::f32::consts::FRAC_PI_2;
use std::sync::Arc;

use potpot::glam::{Mat4, Vec3};
use potpot::parts::{self, Button, Meter, Pad, PushSwitch};
use potpot::{Light, Look, Material, Object, Scene};
use potpot_controls::interaction::Push;

use super::{SEAT, SIZE, framed, screws};

/// Returns rubber buttons and level meters on a dark panel.
///
/// Some of the buttons are lit, the meters stand at a few levels and the pad's puck is off its
/// middle. The light is dimmed so that what glows shows.
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
  let mut objects = vec![Object {
    mesh: Arc::new(parts::plate(width, height, 6.0)),
    transform: Mat4::IDENTITY,
    material: Material::textured(Vec3::new(0.075, 0.08, 0.09)),
  }];
  let mut lamps = Vec::new();
  let seat = Material::matte(SEAT);

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
      transform: shape.transform(x, y, held),
      material: rubber.lit(color, level, shape.emission_radius()),
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

  // push switches out, latched in and held all the way in: square ones that light when
  // on, then round black ones
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
        rubber.lit(amber, level, switch.plunger.emission_radius())
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

  // a pad whose lit puck is most of the way to its top right corner
  let xy = Pad::new(190.0, 130.0);
  let (x, y) = (565.0, 525.0);
  let on_panel = Mat4::from_translation(Vec3::new(x, y, 0.0));
  let on_pad = xy.transform(x, y, 0.8, 0.7);
  let blue = colors[2];
  objects.extend([
    Object {
      mesh: Arc::new(xy.surface()),
      transform: on_panel,
      material: Material::glossy(Vec3::splat(0.02)),
    },
    Object {
      mesh: Arc::new(xy.rim()),
      transform: on_panel,
      material: collar,
    },
    Object {
      mesh: Arc::new(xy.puck()),
      transform: on_pad,
      material: rubber.lit(blue, 1.0, xy.puck.emission_radius()),
    },
  ]);
  let puck = on_pad.transform_point3(Vec3::ZERO);
  lamps.push(xy.puck.lamp(puck.x, puck.y, blue, 1.0));

  objects.extend(screws());
  Scene {
    light: Light {
      color: Vec3::splat(0.6),
      ambient: Vec3::splat(0.2),
      ..Light::default()
    },
    lamps,
    ..framed(look, objects)
  }
}
