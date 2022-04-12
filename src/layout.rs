use std::cmp::{Ord, Ordering, PartialOrd};
use std::convert::TryFrom;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

#[inline]
pub fn try_from(w: usize) -> u16 {
    u16::try_from(w).unwrap()
}
#[inline]
pub fn width(s: &str) -> u16 {
    u16::try_from(s.width()).unwrap()
}
#[inline]
pub fn cwidth(ch: char) -> u16 {
    ch.width().map(|w| w as u16).unwrap_or(0)
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Position {
    pub col: u16, // The leftmost column is number 0.
    pub row: u16, // The highest row is number 0.
}

impl PartialOrd for Position {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Position {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.row.cmp(&other.row) {
            Ordering::Equal => self.col.cmp(&other.col),
            o => o,
        }
    }
}

#[derive(Debug, Default)]
pub struct Layout {
    /// Prompt Unicode/visible width and height
    pub prompt_size: Position,
    pub default_prompt: bool,
    /// Cursor position (relative to the start of the prompt)
    pub cursor: Position,
    /// Number of rows used so far (from start of prompt to end of input)
    pub end: Position,
}
