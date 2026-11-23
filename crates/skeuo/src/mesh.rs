//! triangle meshes and the few shapes a panel is built from

use std::f32::consts::TAU;

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Vertex {
  pub position: [f32; 3],
  pub normal: [f32; 3],
}

/// triangles with a normal at every corner. faces that meet at a hard edge do not share
/// vertices, so each keeps its own normal
#[derive(Clone, Debug, Default)]
pub struct Mesh {
  pub vertices: Vec<Vertex>,
  pub indices: Vec<u32>,
}

impl Mesh {
  /// a box between two corners, flat on every side
  pub fn cuboid(min: Vec3, max: Vec3) -> Self {
    let mut mesh = Self::default();
    for axis in 0..3 {
      for (side, sign) in [(min, -1.0), (max, 1.0)] {
        let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
        let mut normal = Vec3::ZERO;
        normal[axis] = sign;
        let corner = |a: f32, b: f32| {
          let mut position = Vec3::ZERO;
          position[axis] = side[axis];
          position[u] = a;
          position[v] = b;
          position
        };
        mesh.quad(
          [
            corner(min[u], min[v]),
            corner(max[u], min[v]),
            corner(max[u], max[v]),
            corner(min[u], max[v]),
          ],
          normal,
        );
      }
    }
    mesh
  }

  /// a profile of (radius, height) points turned around the z axis. the profile runs from the
  /// bottom outside edge upward and in toward the axis, and every stretch between two points
  /// is its own band with a hard edge to the next. `segments` is how many sides the turn has,
  /// and `smooth` rounds the shading across them
  pub fn lathe(profile: &[[f32; 2]], segments: u32, smooth: bool) -> Self {
    let mut mesh = Self::default();
    let around = |step: u32| {
      let (sin, cos) = (step as f32 / segments as f32 * TAU).sin_cos();
      Vec3::new(cos, sin, 0.0)
    };
    for band in profile.windows(2) {
      let ([r0, z0], [r1, z1]) = (band[0], band[1]);
      // the profile's tangent turned a quarter, which faces out of the solid
      let (out, up) = {
        let (dr, dz) = (r1 - r0, z1 - z0);
        let length = dr.hypot(dz).max(f32::EPSILON);
        (dz / length, -dr / length)
      };
      for step in 0..segments {
        let (a, b) = (around(step), around(step + 1));
        let at = |dir: Vec3, r: f32, z: f32| dir * r + Vec3::Z * z;
        let facing = |dir: Vec3| dir * out + Vec3::Z * up;
        let corners = [at(a, r0, z0), at(b, r0, z0), at(b, r1, z1), at(a, r1, z1)];
        if smooth {
          mesh.quad_with_normals(corners, [facing(a), facing(b), facing(b), facing(a)]);
        } else {
          let middle = (a + b).normalize();
          mesh.quad(corners, facing(middle));
        }
      }
    }
    mesh
  }

  /// a closed band of surface through rings of points: `point(ring, angle)` places the point
  /// of a ring at an angle around the z axis, for `rings` rings from the bottom outside up
  /// and in, each of `segments` points. where neighboring faces turn from each other by less
  /// than the angle whose cosine is `crease` the shading rounds over the edge between them,
  /// and anything sharper stays a hard edge. this is [`Mesh::lathe`] for shapes that are not
  /// the same all the way around
  pub fn rings(
    rings: usize,
    segments: u32,
    crease: f32,
    point: impl Fn(usize, f32) -> Vec3,
  ) -> Self {
    let segments = segments as usize;
    let grid: Vec<Vec<Vec3>> = (0..rings)
      .map(|ring| {
        (0..segments)
          .map(|step| point(ring, step as f32 / segments as f32 * TAU))
          .collect()
      })
      .collect();
    let corners = |band: usize, step: usize| {
      let next = (step + 1) % segments;
      [
        grid[band][step],
        grid[band][next],
        grid[band + 1][next],
        grid[band + 1][step],
      ]
    };
    let bands = rings.saturating_sub(1);
    // across the diagonals, which still gives a face where a ring closes to a point
    let faces: Vec<Vec<Vec3>> = (0..bands)
      .map(|band| {
        (0..segments)
          .map(|step| {
            let [a, b, c, d] = corners(band, step);
            (c - a).cross(d - b).normalize_or_zero()
          })
          .collect()
      })
      .collect();

    let mut mesh = Self::default();
    for band in 0..bands {
      for step in 0..segments {
        let own = faces[band][step];
        // the faces around one corner: this one, its neighbor around, and the two in the
        // band past the corner's ring
        let rounded = |ring: usize, around: usize| {
          let mut sum = Vec3::ZERO;
          for near_band in [ring.checked_sub(1), (ring < bands).then_some(ring)] {
            let Some(near_band) = near_band else {
              continue;
            };
            for near_step in [(around + segments - 1) % segments, around % segments] {
              let face = faces[near_band][near_step];
              if face.dot(own) >= crease {
                sum += face;
              }
            }
          }
          // a face squeezed to nothing has no way to face, and neither do its corners
          sum
            .try_normalize()
            .or(own.try_normalize())
            .unwrap_or(Vec3::Z)
        };
        mesh.quad_with_normals(
          corners(band, step),
          [
            rounded(band, step),
            rounded(band, step + 1),
            rounded(band + 1, step + 1),
            rounded(band + 1, step),
          ],
        );
      }
    }
    mesh
  }

  /// the mesh moved by a transform, to bake a part into place before merging
  #[must_use]
  pub fn transformed(mut self, transform: Mat4) -> Self {
    for vertex in &mut self.vertices {
      vertex.position = transform.transform_point3(vertex.position.into()).into();
      vertex.normal = transform
        .transform_vector3(vertex.normal.into())
        .normalize_or_zero()
        .into();
    }
    self
  }

  /// both meshes as one
  #[must_use]
  pub fn merged(mut self, other: &Self) -> Self {
    let base = self.vertices.len() as u32;
    self.vertices.extend_from_slice(&other.vertices);
    self
      .indices
      .extend(other.indices.iter().map(|index| base + index));
    self
  }

  fn quad(&mut self, corners: [Vec3; 4], normal: Vec3) {
    self.quad_with_normals(corners, [normal; 4]);
  }

  pub(crate) fn quad_with_normals(&mut self, corners: [Vec3; 4], normals: [Vec3; 4]) {
    let base = self.vertices.len() as u32;
    for (position, normal) in corners.into_iter().zip(normals) {
      self.vertices.push(Vertex {
        position: position.into(),
        normal: normal.into(),
      });
    }
    self
      .indices
      .extend([0, 1, 2, 0, 2, 3].map(|corner| base + corner));
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_cuboid_has_six_sides_facing_out() {
    let mesh = Mesh::cuboid(Vec3::splat(-1.0), Vec3::splat(1.0));
    assert_eq!(mesh.vertices.len(), 24);
    assert_eq!(mesh.indices.len(), 36);
    for vertex in &mesh.vertices {
      let (position, normal) = (Vec3::from(vertex.position), Vec3::from(vertex.normal));
      assert!(position.dot(normal) > 0.0, "{position} faces {normal}");
    }
  }

  #[test]
  fn a_lathe_faces_away_from_its_axis_and_up_on_top() {
    // a cylinder: up the wall, then in across the top
    let mesh = Mesh::lathe(&[[1.0, 0.0], [1.0, 2.0], [0.0, 2.0]], 8, true);
    assert_eq!(mesh.indices.len(), 2 * 8 * 6);
    for vertex in &mesh.vertices {
      let (position, normal) = (Vec3::from(vertex.position), Vec3::from(vertex.normal));
      if normal.z > 0.5 {
        assert_eq!(position.z, 2.0);
      } else {
        let outward = Vec3::new(position.x, position.y, 0.0);
        assert!(outward.dot(normal) > 0.99, "{position} faces {normal}");
      }
    }
  }

  #[test]
  fn rings_round_over_shallow_edges_and_keep_sharp_ones() {
    // a cylinder of many sides: round around the wall, hard where the wall meets the top
    let shape = [[1.0, 0.0], [1.0, 2.0], [0.0, 2.0]];
    let point = |ring: usize, angle: f32| {
      let [radius, height] = shape[ring];
      Vec3::new(angle.cos() * radius, angle.sin() * radius, height)
    };
    let mesh = Mesh::rings(3, 32, 0.8, point);
    assert_eq!(mesh.indices.len(), 2 * 32 * 6);
    for vertex in &mesh.vertices {
      let (position, normal) = (Vec3::from(vertex.position), Vec3::from(vertex.normal));
      if normal.z > 0.5 {
        assert!((normal.z - 1.0).abs() < 1e-4, "{position} faces {normal}");
      } else {
        // rounded, so a wall corner faces straight out from the axis and not along a face
        let outward = Vec3::new(position.x, position.y, 0.0).normalize();
        assert!(outward.dot(normal) > 0.9999, "{position} faces {normal}");
      }
    }
    // with few sides every edge is sharper than the crease, and each face keeps its own
    let faceted = Mesh::rings(3, 6, 0.8, point);
    let wall = &faceted.vertices[..4];
    assert!(wall.iter().all(|vertex| vertex.normal == wall[0].normal));
  }

  #[test]
  fn merging_keeps_every_triangle_on_its_own_vertices() {
    let part = Mesh::cuboid(Vec3::ZERO, Vec3::ONE);
    let moved = part
      .clone()
      .transformed(Mat4::from_translation(Vec3::X * 5.0));
    let both = part.merged(&moved);
    assert_eq!(both.vertices.len(), 48);
    assert_eq!(both.indices[36..].iter().min(), Some(&24));
    assert_eq!(both.vertices[24].position[0], 5.0);
  }
}
