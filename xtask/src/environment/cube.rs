//! Cube maps of light on the CPU, and their prefiltering for rough highlights.

use std::f32::consts::PI;

use glam::{Vec2, Vec3};

/// The faces in the order a GPU stores them: +x, -x, +y, -y, +z, -z.
pub const FACES: usize = 6;

/// Returns the way out through a point of a face.
///
/// `at` runs from -1 to 1 across the face to the right and down it, as the GPU's cube lookup
/// lays each face out.
pub fn direction(face: usize, at: Vec2) -> Vec3 {
  let Vec2 { x: u, y: v } = at;
  let out = match face {
    0 => Vec3::new(1.0, -v, -u),
    1 => Vec3::new(-1.0, -v, u),
    2 => Vec3::new(u, 1.0, v),
    3 => Vec3::new(u, -1.0, -v),
    4 => Vec3::new(u, -v, 1.0),
    5 => Vec3::new(-u, -v, -1.0),
    _ => unreachable!("a cube has six faces"),
  };
  out.normalize()
}

/// Returns the face a way out goes through, and where on it, as [`direction`] takes them.
pub fn located(out: Vec3) -> (usize, Vec2) {
  let Vec3 { x, y, z } = out;
  let (ax, ay, az) = (x.abs(), y.abs(), z.abs());
  if ax >= ay && ax >= az {
    if x > 0.0 {
      (0, Vec2::new(-z, -y) / ax)
    } else {
      (1, Vec2::new(z, -y) / ax)
    }
  } else if ay >= az {
    if y > 0.0 {
      (2, Vec2::new(x, z) / ay)
    } else {
      (3, Vec2::new(x, -z) / ay)
    }
  } else if z > 0.0 {
    (4, Vec2::new(x, -y) / az)
  } else {
    (5, Vec2::new(-x, -y) / az)
  }
}

/// One level of a cube map: six square faces of linear light, in rows from the top.
#[derive(Clone, Debug)]
pub struct Level {
  pub side: usize,
  pub texels: Vec<Vec3>,
}

impl Level {
  /// Creates a level by asking `light` for each texel's middle.
  pub fn from_fn(side: usize, light: impl Fn(Vec3) -> Vec3 + Sync) -> Self {
    let mut texels = vec![Vec3::ZERO; FACES * side * side];
    in_parallel(&mut texels, side, |face, at| light(direction(face, at)));
    Self { side, texels }
  }

  /// Returns the level half as wide, each texel the mean of the four under it.
  fn halved(&self) -> Self {
    let side = self.side / 2;
    let mut texels = Vec::with_capacity(FACES * side * side);
    for face in 0..FACES {
      for y in 0..side {
        for x in 0..side {
          let texel =
            |dx, dy| self.texels[(face * self.side + y * 2 + dy) * self.side + x * 2 + dx];
          texels.push((texel(0, 0) + texel(1, 0) + texel(0, 1) + texel(1, 1)) / 4.0);
        }
      }
    }
    Self { side, texels }
  }

  /// Returns the light out along a way, mixed from the four nearest texels of its face.
  fn sampled(&self, out: Vec3) -> Vec3 {
    let (face, at) = located(out);
    let side = self.side as f32;
    let place = ((at + 1.0) * 0.5 * side - 0.5).clamp(Vec2::ZERO, Vec2::splat(side - 1.0));
    let (x0, y0) = (place.x.floor() as usize, place.y.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(self.side - 1), (y0 + 1).min(self.side - 1));
    let part = place - place.floor();
    let texel = |x: usize, y: usize| self.texels[(face * self.side + y) * self.side + x];
    let top = texel(x0, y0).lerp(texel(x1, y0), part.x);
    let foot = texel(x0, y1).lerp(texel(x1, y1), part.x);
    top.lerp(foot, part.y)
  }
}

/// A cube map with every level down to one texel a side.
#[derive(Debug)]
pub struct Cube {
  levels: Vec<Level>,
}

impl Cube {
  /// Creates the levels under `top`, which is a power of two wide.
  pub fn new(top: Level) -> Self {
    let mut levels = vec![top];
    while levels[levels.len() - 1].side > 1 {
      let next = levels[levels.len() - 1].halved();
      levels.push(next);
    }
    Self { levels }
  }

  /// Returns the light out along a way at a level of detail, mixed between the two nearest.
  fn sampled(&self, out: Vec3, lod: f32) -> Vec3 {
    let last = (self.levels.len() - 1) as f32;
    let lod = lod.clamp(0.0, last);
    let below = lod.floor() as usize;
    let above = (below + 1).min(self.levels.len() - 1);
    self.levels[below]
      .sampled(out)
      .lerp(self.levels[above].sampled(out), lod - lod.floor())
  }

  /// Returns a level `side` texels wide of this map seen in a surface of a roughness.
  ///
  /// Each texel is the mean of `samples` ways spread by the GGX distribution around it, as
  /// Karis's split sum takes them with the view along the normal, each weighed by n.l and read
  /// from a level as coarse as the ways are sparse. `roughness` is perceptual.
  pub fn prefiltered(&self, side: usize, roughness: f32, samples: u32) -> Level {
    let alpha = roughness * roughness;
    let top = self.levels[0].side as f32;
    let texel_angle = 4.0 * PI / (FACES as f32 * top * top);
    let mut texels = vec![Vec3::ZERO; FACES * side * side];
    in_parallel(&mut texels, side, |face, at| {
      let normal = direction(face, at);
      if alpha == 0.0 {
        return self.sampled(normal, (top / side as f32).log2());
      }
      let (tangent, bitangent) = normal.any_orthonormal_pair();
      let mut sum = Vec3::ZERO;
      let mut weight = 0.0;
      for index in 0..samples {
        let (half, noh) = ggx_half(hammersley(index, samples), alpha);
        let half = tangent * half.x + bitangent * half.y + normal * half.z;
        let toward = 2.0 * normal.dot(half) * half - normal;
        let nol = normal.dot(toward);
        if nol <= 0.0 {
          continue;
        }
        // the pdf of `toward` with the view along the normal is D / 4
        let pdf = ggx(noh, alpha) / 4.0;
        let sample_angle = 1.0 / (samples as f32 * pdf);
        let lod = 0.5 * (sample_angle / texel_angle).log2() + 1.0;
        sum += self.sampled(toward, lod) * nol;
        weight += nol;
      }
      sum / weight
    });
    Level { side, texels }
  }
}

/// Fills six faces `side` wide, row by row across the CPU's threads.
fn in_parallel(texels: &mut [Vec3], side: usize, light: impl Fn(usize, Vec2) -> Vec3 + Sync) {
  let threads = std::thread::available_parallelism().map_or(1, |count| count.get());
  let rows_per_chunk = (FACES * side).div_ceil(threads).max(1);
  std::thread::scope(|scope| {
    for (chunk, rows) in texels.chunks_mut(rows_per_chunk * side).enumerate() {
      let light = &light;
      scope.spawn(move || {
        for (index, texel) in rows.iter_mut().enumerate() {
          let row = chunk * rows_per_chunk + index / side;
          let (face, y, x) = (row / side, row % side, index % side);
          let at = (Vec2::new(x as f32, y as f32) + 0.5) / side as f32 * 2.0 - 1.0;
          *texel = light(face, at);
        }
      });
    }
  });
}

/// Returns the GGX distribution of facets at n.h for a roughness squared of `alpha`.
fn ggx(noh: f32, alpha: f32) -> f32 {
  let a2 = alpha * alpha;
  let f = noh * noh * (a2 - 1.0) + 1.0;
  a2 / (PI * f * f)
}

/// Returns a facet normal drawn from the GGX distribution by a point of the unit square.
///
/// The normal is around +z, and comes with its z.
fn ggx_half(point: Vec2, alpha: f32) -> (Vec3, f32) {
  let turn = 2.0 * PI * point.x;
  let cos = ((1.0 - point.y) / (1.0 + (alpha * alpha - 1.0) * point.y)).sqrt();
  let sin = (1.0 - cos * cos).max(0.0).sqrt();
  (Vec3::new(sin * turn.cos(), sin * turn.sin(), cos), cos)
}

/// Returns the `index`th of `count` points of the Hammersley set over the unit square.
fn hammersley(index: u32, count: u32) -> Vec2 {
  Vec2::new(
    index as f32 / count as f32,
    index.reverse_bits() as f32 / 4_294_967_296.0,
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn each_face_point_is_found_where_it_was() {
    for face in 0..FACES {
      for at in [Vec2::new(0.3, -0.7), Vec2::new(-0.9, 0.2), Vec2::ZERO] {
        let (found, back) = located(direction(face, at));
        assert_eq!(found, face);
        assert!(
          (back - at).length() < 1e-5,
          "face {face} {at} came back {back}"
        );
      }
    }
  }

  #[test]
  fn a_constant_studio_prefilters_to_itself_at_every_roughness() {
    let light = Vec3::new(0.8, 1.7, 3.1);
    let cube = Cube::new(Level::from_fn(32, |_| light));
    for mip in 0..6 {
      let level = cube.prefiltered(16 >> mip.min(3), mip as f32 / 5.0, 64);
      for texel in level.texels {
        assert!(
          ((texel - light) / light).abs().max_element() < 1e-3,
          "mip {mip} gave {texel}"
        );
      }
    }
  }
}
