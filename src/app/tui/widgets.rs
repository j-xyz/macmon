//! History graph and gauge widgets.
//!
//! Both are drawn here rather than with ratatui's `Sparkline` / `Gauge`: `Sparkline` rounds bars
//! down (a small non-zero sample stays blank) and scales to the largest sample of all its data
//! (not only the columns on screen), and `Gauge` with an empty label still paints a reversed blank
//! cell in the middle of its middle row.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

use super::theme::gradient;

/// Bar heights in one row: `▁` (1/8) up to `█` (8/8).
const BAR_LEVELS: u64 = 8;
/// Code point before `▁`: the bar of level `n` is `BAR_BASE + n`.
const BAR_BASE: u32 = 0x2580;
/// Filled cell of a gauge, and of a full bar.
const FULL: char = '█';
/// Half-filled cell of a three-level bar.
const HALF: char = '▄';

/// Gauge view of the original macmon: a bar across its whole area, filled from the left to `ratio`
/// (rounded to whole cells) in the load color of `ratio`; the rest stays blank.
pub(super) struct Gauge {
  ratio: f64,
}

impl Gauge {
  /// Gauge for `ratio` in `0.0..=1.0`, clamped outside the range.
  pub(super) fn new(ratio: f64) -> Self {
    Self { ratio: if ratio.is_nan() { 0.0 } else { ratio.clamp(0.0, 1.0) } }
  }
}

impl Widget for Gauge {
  fn render(self, area: Rect, buf: &mut Buffer) {
    let area = area.intersection(buf.area);
    // at most `area.width`, as the ratio is at most 1
    let filled = (f64::from(area.width) * self.ratio).round() as u16;
    let color = gradient(self.ratio);
    for y in area.top()..area.bottom() {
      for x in area.left()..area.left() + filled {
        buf[(x, y)].set_char(FULL).set_fg(color);
      }
    }
  }
}

/// History graph for newest-first samples over its whole area, right-aligned (newest sample on
/// the right): one solid bar per column, growing from the bottom row up in eighths of a row
/// (`▁`…`█`; blank / `▄` / `█` with `three_levels`), each colored by its own value on the load
/// gradient, or a separate value range. Zero values leave the column blank.
pub(super) struct Graph<'a> {
  data: &'a [u64],
  max: Option<u64>,
  color_range: Option<(u64, u64)>,
  three_levels: bool,
}

impl<'a> Graph<'a> {
  /// Graph scaled to its largest visible sample, each bar in its load color.
  pub(super) fn new(data: &'a [u64]) -> Self {
    Self { data, max: None, color_range: None, three_levels: false }
  }

  /// Bars in three levels (blank, `▄`, `█`) instead of eighths, for Apple Terminal, which draws
  /// gaps between the eighth blocks.
  pub(super) fn three_levels(mut self, three_levels: bool) -> Self {
    self.three_levels = three_levels;
    self
  }

  /// Value drawn at full height.
  pub(super) fn max(mut self, max: u64) -> Self {
    self.max = Some(max);
    self
  }

  /// Color range independent of graph height, e.g. 30–100°C for temperature.
  pub(super) fn color_range(mut self, min: u64, max: u64) -> Self {
    self.color_range = Some((min, max));
    self
  }
}

impl Widget for Graph<'_> {
  fn render(self, area: Rect, buf: &mut Buffer) {
    let area = area.intersection(buf.area);
    if area.is_empty() {
      return;
    }

    let visible = &self.data[..self.data.len().min(usize::from(area.width))];
    let max = self.max.unwrap_or_else(|| visible.iter().copied().max().unwrap_or(0)).max(1);
    let levels = u64::from(area.height) * BAR_LEVELS;
    for (x, &value) in (area.left()..area.right()).rev().zip(visible) {
      let mut level = bar_level(value, max, levels);
      let (low, high) = self.color_range.unwrap_or((0, max));
      let color = gradient((value as f64 - low as f64) / high.saturating_sub(low).max(1) as f64);
      for y in (area.top()..area.bottom()).rev() {
        if level == 0 {
          break;
        }

        let eighths = level.min(BAR_LEVELS);
        let symbol = bar_symbol(eighths, self.three_levels);
        if symbol != ' ' {
          buf[(x, y)].set_char(symbol).set_fg(color);
        }
        level -= eighths;
      }
    }
  }
}

/// Glyph of a cell filled `eighths` of a row from the bottom: `▁`…`█`, or with three levels (as
/// the original macmon in Apple Terminal) blank up to 1/8, `▄` up to 6/8 and `█` above.
fn bar_symbol(eighths: u64, three_levels: bool) -> char {
  match (three_levels, eighths) {
    (_, 0) | (true, 1) => ' ',
    (true, 2..=6) => HALF,
    (true, _) => FULL,
    (false, _) => char::from_u32(BAR_BASE + eighths.min(BAR_LEVELS) as u32).unwrap_or(' '),
  }
}

/// Bar height of `value` in eighths of a row out of `levels`, rounded up: non-zero values get at
/// least `▁`.
fn bar_level(value: u64, max: u64, levels: u64) -> u64 {
  if value == 0 {
    return 0;
  }

  // in u128, so large values don't overflow
  let level = (u128::from(value) * u128::from(levels)).div_ceil(u128::from(max.max(1)));
  level.min(u128::from(levels)) as u64
}

#[cfg(test)]
mod tests {
  use ratatui::buffer::Buffer;
  use ratatui::layout::Rect;
  use ratatui::style::Color;
  use ratatui::widgets::Widget;

  use super::{Gauge, Graph};

  fn draw(widget: impl Widget, width: u16, height: u16) -> Buffer {
    let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
    widget.render(buf.area, &mut buf);
    buf
  }

  fn rows(buf: &Buffer) -> Vec<String> {
    let row = |y| (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect();
    (0..buf.area.height).map(row).collect()
  }

  #[test]
  fn graph_bars_grow_across_rows() {
    // three rows, newest on the right: 100 % full, 50 % one and a half rows, 30 % (7.2 eighths
    // rounded up to 8) one row, a tiny value one eighth at the bottom, zero blank
    let buf = draw(Graph::new(&[100, 50, 30, 1, 0]).max(100), 5, 3);
    assert_eq!(rows(&buf), ["█", "▄█", "▁███"].map(|r| format!("{r:>5}")));

    // every level of a two-row bar, bottom row full before the top row starts
    let levels: Vec<u64> = (1..=16).rev().collect();
    let buf = draw(Graph::new(&levels).max(16), 16, 2);
    assert_eq!(rows(&buf), ["        ▁▂▃▄▅▆▇█", "▁▂▃▄▅▆▇█████████"]);

    // without a maximum: scaled to the largest sample on screen, the older 100 doesn't fit
    assert_eq!(rows(&draw(Graph::new(&[10, 20, 100]), 2, 2)), ["█ ", "██"]);
  }

  #[test]
  fn graph_colors_each_column_by_load_or_temperature() {
    // the terminal's green / yellow / red, by each column's own value
    let buf = draw(Graph::new(&[90, 50, 10]).max(100), 3, 4);
    for (x, color) in [(0, Color::Green), (1, Color::Yellow), (2, Color::Red)] {
      for y in (0..4).filter(|&y| buf[(x, y)].symbol() != " ") {
        assert_eq!(buf[(x, y)].fg, color, "{x}, {y}");
      }
    }

    // temperature bars: the 30–100°C colors are independent of the 0–150°C height.
    let buf = draw(
      Graph::new(&[100_000, 65_000, 0, 30_000]).max(150_000).color_range(30_000, 100_000),
      4,
      2,
    );
    assert_eq!(rows(&buf), ["   ▃", "▄ ▇█"]);
    for (x, color) in [(0, Color::Green), (2, Color::Yellow), (3, Color::Red)] {
      assert_eq!(buf[(x, 1)].fg, color);
    }
  }

  #[test]
  fn fixed_temperature_scale_does_not_change_with_visible_history() {
    let temps = [120_000, 60_000, 150_000];
    let narrow = draw(Graph::new(&temps).max(150_000), 2, 5);
    let wide = draw(Graph::new(&temps).max(150_000), 3, 5);
    for y in 0..5 {
      for x in 0..2 {
        assert_eq!(narrow[(x, y)], wide[(x + 1, y)]);
      }
    }
    // 120°C is higher than 60°C and still below the 150°C display ceiling.
    assert_eq!(rows(&narrow), ["  ", " █", " █", "██", "██"]);
    assert_eq!(rows(&wide)[0], "█  ");
  }

  #[test]
  fn graph_in_three_levels_for_apple_terminal() {
    // the bar set of the original macmon in Apple Terminal: 1/8 blank, 2/8–6/8 `▄`, 7/8–8/8 `█`,
    // full cells below the top one
    let levels: Vec<u64> = (1..=16).rev().collect();
    let buf = draw(Graph::new(&levels).max(16).three_levels(true), 16, 2);
    assert_eq!(rows(&buf), ["         ▄▄▄▄▄██", " ▄▄▄▄▄██████████"]);
  }

  #[test]
  fn gauge_fills_its_ratio_of_every_row() {
    // (ratio, width, filled cells): rounded to whole cells, clamped to the range
    let cases =
      [(0.0, 10, 0), (0.5, 10, 5), (0.556, 48, 27), (0.04, 10, 0), (1.5, 4, 4), (f64::NAN, 4, 0)];
    for (ratio, width, filled) in cases {
      let buf = draw(Gauge::new(ratio), width, 3);
      let row = format!("{}{}", "█".repeat(filled), " ".repeat(usize::from(width) - filled));
      assert_eq!(rows(&buf), vec![row; 3], "{ratio} in {width}");
    }
    // in the load color of the ratio
    assert_eq!(draw(Gauge::new(0.5), 4, 1)[(0, 0)].fg, Color::Yellow);
  }
}
