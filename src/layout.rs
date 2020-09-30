use std::cmp::{Ord, Ordering, PartialOrd};

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;


#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Position {
    pub col: usize, // The leftmost column is number 0.
    pub row: usize, // The highest row is number 0.
}

#[derive(Debug, PartialEq, Clone)]
enum EscapeSeqState {
    Initial,
    EscapeChar,
    BracketSequence,
}

#[derive(Debug, Clone)]
pub struct Meter {
    position: Position,
    cols: usize,
    tab_stop: usize,
    left_margin: usize,
    escape_seq_state: EscapeSeqState,
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
    /// Cursor position (relative to the end of the prompt)
    pub cursor: Position,
    /// Number of rows used so far (from end of prompt to end of input)
    pub end: Position,
    /// Number of first visible row
    pub scroll_top: usize,
    /// Number of visitble rows (not hidden by scroll)
    pub screen_rows: usize,
}

impl Meter {
    pub fn new(cols: usize, tab_stop: usize) -> Meter {
        Meter {
            position: Position::default(),
            cols,
            tab_stop,
            left_margin: 0,
            escape_seq_state: EscapeSeqState::Initial,
        }
    }
    pub fn left_margin(&mut self, value: usize) -> &mut Self {
        debug_assert!(value < self.cols);
        self.left_margin = value;
        self
    }
    pub fn set_position(&mut self, pos: Position) {
        self.position = pos;
    }
    pub fn get_position(&self) -> Position {
        self.position
    }
    pub fn get_row(&self) -> usize {
        self.position.row
    }
    /// Control characters are treated as having zero width.
    /// Characters with 2 column width are correctly handled (not split).
    pub fn update(&mut self, text: &str) -> Position {
        let mut pos = self.position;
        for c in text.graphemes(true) {
            if c == "\n" {
                pos.row += 1;
                pos.col = self.left_margin;
                continue;
            }
            let cw = if c == "\t" {
                self.tab_stop - (pos.col % self.tab_stop)
            } else {
                self.char_width(c)
            };
            pos.col += cw;
            if pos.col > self.cols {
                pos.row += 1;
                pos.col = cw;
            }
        }
        if pos.col == self.cols {
            pos.col = 0;
            pos.row += 1;
        }
        if self.escape_seq_state != EscapeSeqState::Initial {
            log::warn!("unfinished escape sequence in {:?}", text);
            self.escape_seq_state = EscapeSeqState::Initial;
        }
        self.position = pos;
        pos
    }
    /// Same as update, but only updates up to a visual line be it
    /// the number of columns filled or a newline character
    ///
    /// Returns the index of the newline character or the first character
    /// that doesn't fit a line or None if whole text fits.
    pub fn update_line(&mut self, text: &str) -> Option<usize> {
        for (idx, c) in text.grapheme_indices(true) {
            if c == "\n" {
                return Some(idx);
            }
            let cw = if c == "\t" {
                self.tab_stop - (self.position.col % self.tab_stop)
            } else {
                self.char_width(c)
            };
            if self.position.col + cw > self.cols {
                return Some(idx);
            }
            self.position.col += cw;
        }
        if self.escape_seq_state != EscapeSeqState::Initial {
            log::warn!("unfinished escape sequence in {:?}", text);
            self.escape_seq_state = EscapeSeqState::Initial;
        }
        return None;
    }
    /// A faster equivalent of self.update("\n");
    pub fn update_newline(&mut self) {
        self.position.row += 1;
        self.position.col = self.left_margin;
    }
    // ignore ANSI escape sequence
    fn char_width(&mut self, s: &str) -> usize {
        use EscapeSeqState::*;

        if self.escape_seq_state == EscapeChar {
            if s == "[" {
                // CSI
                self.escape_seq_state = BracketSequence;
            } else {
                // two-character sequence
                self.escape_seq_state = Initial;
            }
            0
        } else if self.escape_seq_state == BracketSequence {
            if s == ";" || (s.as_bytes()[0] >= b'0' && s.as_bytes()[0] <= b'9')
            {
                /*} else if s == "m" {
                // last
                 *esc_seq = 0;*/
            } else {
                // not supported
                self.escape_seq_state = Initial;
            }
            0
        } else if s == "\x1b" {
            self.escape_seq_state = EscapeChar;
            0
        } else if s == "\n" {
            0
        } else {
            s.width()
        }
    }
}

impl Layout {
    /// Returns number of visible on the screen below the cursor
    pub fn lines_below_cursor(&self) -> usize {
        if self.end.row < self.screen_rows || self.screen_rows == 0 {
            self.end.row - self.cursor.row
        } else {
            self.scroll_top + self.screen_rows - self.cursor.row - 1
        }
    }
}

#[test]
#[ignore]
fn prompt_with_ansi_escape_codes() {
    let pos = Meter::new(80, 4).update("\x1b[1;32m>>\x1b[0m ");
    assert_eq!(3, pos.col);
    assert_eq!(0, pos.row);
}
