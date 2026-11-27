// the cells of a display's face, which its light shows through: a lit square in each, with a
// dark gap between

// the share of a display's cell its lit square takes across
const CELL_FILL: f32 = 0.78;
// the share of a display's light that shows in the gaps between its cells
const GAP_LIGHT: f32 = 0.3;

// how much of a display's light a pixel shows, the pixel at `across` and `wide` across, both
// in cells. it is the pixel's mean, so a gap thinner than a pixel shows by how much of the
// pixel it takes wherever it falls, the same across and down
fn cells_over(across: vec2<f32>, wide: vec2<f32>) -> f32 {
  let wide_enough = max(wide, vec2<f32>(1e-4));
  let lit = lit_share_of_cells(across.x, wide_enough.x) * lit_share_of_cells(across.y, wide_enough.y);
  return mix(GAP_LIGHT, 1.0, lit);
}

// the share of a stretch `wide` cells long, about `at`, that a row of cells' lit squares
// covers
fn lit_share_of_cells(at: f32, wide: f32) -> f32 {
  return (lit_up_to(at + wide * 0.5) - lit_up_to(at - wide * 0.5)) / wide;
}

// the length of lit square from zero up to `at`, in cells. each cell's square is centered in it
fn lit_up_to(at: f32) -> f32 {
  let gap = (1.0 - CELL_FILL) * 0.5;
  return floor(at) * CELL_FILL + clamp(fract(at) - gap, 0.0, CELL_FILL);
}
