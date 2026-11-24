//! a plain drawn face of capitals and figures, as the strokes its letters are made of. what
//! draws them, flat or as a solid, is whoever uses it

/// a place, across and down
pub type Point = [f32; 2];

/// the strokes of a letter on a grid four wide and six tall: each run of digit pairs is a
/// line through those points, across and then up from the letter's foot
fn strokes(letter: char) -> &'static [&'static str] {
  match letter {
    'A' => &["002640", "1232"],
    'B' => &["00063645443303", "3342413000"],
    'C' => &["4536160501103041"],
    'D' => &["00063645413000"],
    'E' => &["46060040", "0333"],
    'F' => &["460600", "0333"],
    'G' => &["45361605011030414323"],
    'H' => &["0006", "4046", "0343"],
    'I' => &["2026", "1030", "1636"],
    'J' => &["0110304146"],
    'K' => &["0006", "460340"],
    'L' => &["060040"],
    'M' => &["0006234640"],
    'N' => &["00064046"],
    'O' | '0' => &["160501103041453616"],
    'P' => &["00063645443303"],
    'Q' => &["160501103041453616", "2240"],
    'R' => &["00063645443303", "2340"],
    'S' => &["453616050413334241301001"],
    'T' => &["0646", "2620"],
    'U' => &["060110304146"],
    'V' => &["062046"],
    'W' => &["0610233046"],
    'X' => &["0046", "0640"],
    'Y' => &["062346", "2320"],
    'Z' => &["06460040"],
    '1' => &["152620", "1030"],
    '2' => &["05163645440040"],
    '3' => &["05163645443323", "334241301001"],
    '4' => &["30360242"],
    '5' => &["460603334241301001"],
    '6' => &["4536160501103041423303"],
    '7' => &["064610"],
    '8' => &["1304051636454433", "1302011030414233"],
    '9' => &["0110304145361605041343"],
    '-' => &["1333"],
    '+' => &["1333", "2224"],
    '.' => &["2020"],
    ',' => &["2110"],
    ':' => &["2121", "2424"],
    '/' => &["0046"],
    '%' => &["0046", "0516251405", "2132413021"],
    '>' => &["052301"],
    '<' => &["450341"],
    _ => &[],
  }
}

/// whether the face has a letter for it. a small letter counts as its capital
pub fn lettered(letter: char) -> bool {
  !strokes(letter.to_ascii_uppercase()).is_empty()
}

/// how far across from the middle of lettering of `count` letters `height` tall the gap
/// before its letter `index` is, where a mark between two letters goes. `index` runs up to
/// `count`, which is the gap after the last letter
pub fn gap(count: usize, index: usize, height: f32) -> f32 {
  let unit = height / 6.0;
  left_edge(count, unit) + (index as f32 * 6.0 - 1.0) * unit
}

/// how wide lettering of `count` letters `height` tall is, from the left of its first letter to
/// the right of its last
pub const fn width(count: usize, height: f32) -> f32 {
  -2.0 * left_edge(count, height / 6.0)
}

/// the left edge of the first of `count` letters from the middle of them all. each letter
/// is four units wide, and two more go to the gap after it
const fn left_edge(count: usize, unit: f32) -> f32 {
  -(count as f32 * 6.0 - 2.0) * unit / 2.0
}

/// the strokes of lettering as lines from point to point, about the middle of it all:
/// letters `height` tall, down the panel as y grows. anything the face has no letter for is
/// a space
pub fn lines(text: &str, height: f32) -> Vec<[Point; 2]> {
  let unit = height / 6.0;
  let left = left_edge(text.chars().count(), unit);
  let mut lines = Vec::new();
  for (index, letter) in text.chars().enumerate() {
    let origin = [left + index as f32 * 6.0 * unit, height / 2.0];
    for run in strokes(letter.to_ascii_uppercase()) {
      let points: Vec<Point> = (run.as_bytes().chunks(2))
        .map(|pair| {
          let (across, up) = (f32::from(pair[0] - b'0'), f32::from(pair[1] - b'0'));
          // up the letter is up the panel, which is toward negative y
          [origin[0] + across * unit, origin[1] + -up * unit]
        })
        .collect();
      lines.extend(points.windows(2).map(|line| [line[0], line[1]]));
    }
  }
  lines
}

/// where the top left of the first letter is from the middle of lettering of `count` letters
/// `height` tall
pub fn top_left(count: usize, height: f32) -> Point {
  [left_edge(count, height / 6.0), -height / 2.0]
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn every_capital_and_figure_has_strokes_on_its_grid() {
    for letter in ' '..='~' {
      let runs = strokes(letter);
      if letter.is_ascii_uppercase() || letter.is_ascii_digit() {
        assert!(!runs.is_empty(), "{letter} is blank");
      }
      for run in runs {
        assert!(run.len() >= 4 && run.len() % 2 == 0, "{letter}: {run}");
        for pair in run.as_bytes().chunks(2) {
          assert!(
            (b'0'..=b'4').contains(&pair[0]) && (b'0'..=b'6').contains(&pair[1]),
            "{letter}: {run}"
          );
        }
      }
    }
  }

  #[test]
  fn the_gaps_of_lettering_lie_between_its_letters() {
    // three letters at two to a unit: each 8 wide with 4 between, 32 in all
    let gaps: Vec<f32> = (0..=3).map(|index| gap(3, index, 12.0)).collect();
    assert_eq!(gaps, [-18.0, -6.0, 6.0, 18.0]);
    // with no letters the one gap is the middle
    assert_eq!(gap(0, 0, 12.0), 0.0);
    assert_eq!(width(3, 12.0), 32.0);
  }
}
