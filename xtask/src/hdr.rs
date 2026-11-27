//! Reading of Radiance `.hdr` pictures.

use potpot::glam::Vec3;

/// A picture of linear light, in rows from the top.
#[derive(Debug)]
pub struct Picture {
  pub width: usize,
  pub height: usize,
  pub pixels: Vec<Vec3>,
}

/// Returns the picture an `.hdr` file holds.
///
/// The file is in the RGBE format, with rows stored flat or run-length encoded.
pub fn read(bytes: &[u8]) -> Result<Picture, String> {
  let mut rest = bytes;
  let (width, height) = loop {
    let end = rest
      .iter()
      .position(|&byte| byte == b'\n')
      .ok_or("the header has no size line")?;
    let line = std::str::from_utf8(&rest[..end]).map_err(|_| "a header line is not text")?;
    rest = &rest[end + 1..];
    if let Some(size) = line.strip_prefix("-Y ") {
      let (height, width) = size
        .split_once(" +X ")
        .ok_or("the size line is not -Y h +X w")?;
      let parse = |number: &str| {
        number
          .trim()
          .parse::<usize>()
          .map_err(|error| error.to_string())
      };
      break (parse(width)?, parse(height)?);
    }
    if line.starts_with("FORMAT=") && line != "FORMAT=32-bit_rle_rgbe" {
      return Err(format!("the pixels are not rgbe: {line}"));
    }
  };
  let mut pixels = Vec::with_capacity(width * height);
  let mut row = vec![[0u8; 4]; width];
  for _ in 0..height {
    rest = read_row(rest, &mut row)?;
    pixels.extend(row.iter().map(|&rgbe| decoded(rgbe)));
  }
  Ok(Picture {
    width,
    height,
    pixels,
  })
}

/// Reads one row into `row`, and returns what follows it.
fn read_row<'a>(bytes: &'a [u8], row: &mut [[u8; 4]]) -> Result<&'a [u8], String> {
  let short = || "the file ends inside a row".to_string();
  let width = row.len();
  let encoded = bytes.len() >= 4
    && bytes[0] == 2
    && bytes[1] == 2
    && usize::from(bytes[2]) << 8 | usize::from(bytes[3]) == width;
  if !encoded {
    let flat = bytes.get(..width * 4).ok_or_else(short)?;
    row.copy_from_slice(flat.as_chunks::<4>().0);
    return Ok(&bytes[width * 4..]);
  }
  // each channel in turn, as runs of one byte repeated or of bytes as they are
  let mut at = 4;
  for channel in 0..4 {
    let mut x = 0;
    while x < width {
      let count = usize::from(*bytes.get(at).ok_or_else(short)?);
      at += 1;
      if count > 128 {
        let count = count - 128;
        let value = *bytes.get(at).ok_or_else(short)?;
        at += 1;
        let run = row
          .get_mut(x..x + count)
          .ok_or("a run passes the row's end")?;
        run.iter_mut().for_each(|texel| texel[channel] = value);
        x += count;
      } else {
        let values = bytes.get(at..at + count).ok_or_else(short)?;
        at += count;
        let run = row
          .get_mut(x..x + count)
          .ok_or("a run passes the row's end")?;
        for (texel, &value) in run.iter_mut().zip(values) {
          texel[channel] = value;
        }
        x += count;
      }
    }
  }
  Ok(&bytes[at..])
}

/// Returns the light a texel of shared-exponent RGBE holds.
fn decoded([r, g, b, e]: [u8; 4]) -> Vec3 {
  if e == 0 {
    return Vec3::ZERO;
  }
  let scale = 2f32.powi(i32::from(e) - 136);
  Vec3::new(f32::from(r), f32::from(g), f32::from(b)) * scale
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn flat_and_encoded_rows_read_alike() {
    let width = 8;
    let texels: Vec<[u8; 4]> = (0..width as u8)
      .map(|x| [x * 9, 200 - x, 7, 128 + x])
      .collect();
    let header = format!("#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n\n-Y 2 +X {width}\n");
    let mut flat = header.clone().into_bytes();
    let mut encoded = header.into_bytes();
    for _ in 0..2 {
      flat.extend(texels.iter().flatten());
      encoded.extend([2, 2, 0, width as u8]);
      for channel in 0..4 {
        // the first three bytes as they are, then the rest as runs of one
        encoded.push(3);
        encoded.extend(texels[..3].iter().map(|texel| texel[channel]));
        for texel in &texels[3..] {
          encoded.extend([129, texel[channel]]);
        }
      }
    }
    let flat = read(&flat).expect("the flat file reads");
    let encoded = read(&encoded).expect("the encoded file reads");
    assert_eq!((flat.width, flat.height), (width, 2));
    assert_eq!(flat.pixels, encoded.pixels);
    assert_eq!(
      flat.pixels[1],
      Vec3::new(9.0, 199.0, 7.0) * 2f32.powi(129 - 136)
    );
  }
}
