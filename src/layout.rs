use std::cmp::{Ord, Ordering, PartialOrd};
use std::convert::TryFrom;
use std::ops::Index;
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
    pub end_input: Position,
    /// Number of rows used so far (from start of prompt to end of input + some
    /// info)
    pub end: Position,
    /// First visible row (such as cursor is visible if prompt + line + some
    /// info > screen height)
    pub first_row: u16, // relative to the start of the prompt (= 0 when all rows are visible)
    /// Last visible row (such as cursor is visible if prompt + line + some info
    /// > screen height)
    pub last_row: u16, // relative to the start of the prompt (= end.row when all rows are visible)
    /// start of ith row => byte offset of prompt / line / info
    pub breaks: Vec<usize>,
}

impl Index<u16> for Layout {
    type Output = usize;

    fn index(&self, index: u16) -> &usize {
        self.breaks.index(index as usize)
    }
}

impl Layout {
    /// Return `true` if we need to scroll to make `cursor` visible
    pub fn scroll(&self, cursor: Position) -> bool {
        self.first_row > cursor.row || self.last_row < cursor.row
    }

    pub fn visible_prompt<'p>(&self, prompt: &'p str) -> &'p str {
        if self.first_row > self.prompt_size.row {
            return ""; // prompt not visible
        } else if self.first_row == 0 {
            return prompt;
        }
        &prompt[self[self.first_row]..]
    }

    pub fn visible_line<'l>(&self, line: &'l str, pos: usize) -> (&'l str, usize) {
        if self.first_row <= self.prompt_size.row {
            if self.end_input.row <= self.last_row {
                return (line, pos);
            }
        } else if self.end_input.row <= self.last_row {
            let offset = self[self.first_row];
            return (&line[offset..], pos.saturating_sub(offset));
        }
        let start = self[self.first_row];
        let end = self[self.last_row];
        (&line[start..end], pos.saturating_sub(start))
    }

    pub fn visible_hint<'h>(&self, hint: &'h str) -> &'h str {
        if self.end.row == self.last_row {
            return hint;
        } else if self.last_row < self.end_input.row {
            return ""; // hint not visible
        }
        let end = self[self.last_row];
        &hint[..end]
    }

    /// Number of visible rows under cursor
    pub fn lines_below_cursor(&self) -> u16 {
        self.last_row.saturating_sub(self.cursor.row)
    }

    pub fn reset_rows(&mut self) {
        self.last_row = 0;
        self.cursor.row = 0;
    }

    pub fn reset(&mut self) {
        self.cursor = Position::default();
        self.end = Position::default();
        self.first_row = 0;
        self.last_row = 0;
    }
}
