//! Line buffer with current cursor position
use std::iter;
use std::ops::{Deref, Range};
use std_unicode::str::UnicodeStr;
use unicode_segmentation::UnicodeSegmentation;
use keymap::{Anchor, At, CharSearch, Movement, RepeatCount, Word};

/// Maximum buffer size for the line read
pub static MAX_LINE: usize = 4096;

pub enum WordAction {
    CAPITALIZE,
    LOWERCASE,
    UPPERCASE,
}

#[derive(Debug)]
pub struct LineBuffer {
    buf: String, // Edited line buffer
    pos: usize, // Current cursor position (byte position)
}

impl LineBuffer {
    /// Create a new line buffer with the given maximum `capacity`.
    pub fn with_capacity(capacity: usize) -> LineBuffer {
        LineBuffer {
            buf: String::with_capacity(capacity),
            pos: 0,
        }
    }

    #[cfg(test)]
    pub fn init(line: &str, pos: usize) -> LineBuffer {
        let mut lb = Self::with_capacity(MAX_LINE);
        assert!(lb.insert_str(0, line));
        lb.set_pos(pos);
        lb
    }

    /// Extracts a string slice containing the entire buffer.
    pub fn as_str(&self) -> &str {
        &self.buf
    }

    /// Converts a buffer into a `String` without copying or allocating.
    pub fn into_string(self) -> String {
        self.buf
    }

    /// Current cursor position (byte position)
    pub fn pos(&self) -> usize {
        self.pos
    }
    pub fn set_pos(&mut self, pos: usize) {
        assert!(pos <= self.buf.len());
        self.pos = pos;
    }

    /// Returns the length of this buffer, in bytes.
    pub fn len(&self) -> usize {
        self.buf.len()
    }
    /// Returns `true` if this buffer has a length of zero.
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Set line content (`buf`) and cursor position (`pos`).
    pub fn update(&mut self, buf: &str, pos: usize) {
        assert!(pos <= buf.len());
        self.buf.clear();
        let max = self.buf.capacity();
        if buf.len() > max {
            self.buf.push_str(&buf[..max]);
            if pos > max {
                self.pos = max;
            } else {
                self.pos = pos;
            }
        } else {
            self.buf.push_str(buf);
            self.pos = pos;
        }
    }

    /// Backup `src`
    pub fn backup(&mut self, src: &LineBuffer) {
        self.buf.clear();
        self.buf.push_str(&src.buf);
        self.pos = src.pos;
    }

    /// Returns the character at current cursor position.
    fn char_at_cursor(&self) -> Option<char> {
        if self.pos == self.buf.len() {
            None
        } else {
            self.buf[self.pos..].chars().next()
        }
    }

    fn next_pos(&self, n: RepeatCount) -> Option<usize> {
        if self.pos == self.buf.len() {
            return None;
        }
        self.buf[self.pos..]
            .grapheme_indices(true)
            .take(n)
            .last()
            .map(|(i, s)| i + self.pos + s.len())
    }
    /// Returns the position of the character just before the current cursor position.
    fn prev_pos(&self, n: RepeatCount) -> Option<usize> {
        if self.pos == 0 {
            return None;
        }
        self.buf[..self.pos]
            .grapheme_indices(true)
            .rev()
            .take(n)
            .last()
            .map(|(i, _)| i)
    }

    /// Insert the character `ch` at current cursor position
    /// and advance cursor position accordingly.
    /// Return `None` when maximum buffer size has been reached,
    /// `true` when the character has been appended to the end of the line.
    pub fn insert(&mut self, ch: char, n: RepeatCount) -> Option<bool> {
        let shift = ch.len_utf8() * n;
        if self.buf.len() + shift > self.buf.capacity() {
            return None;
        }
        let push = self.pos == self.buf.len();
        if push {
            self.buf.reserve(shift);
            for _ in 0..n {
                self.buf.push(ch);
            }
        } else if n == 1 {
            self.buf.insert(self.pos, ch);
        } else {
            let text = iter::repeat(ch).take(n).collect::<String>();
            let pos = self.pos;
            self.insert_str(pos, &text);
        }
        self.pos += shift;
        Some(push)
    }

    /// Yank/paste `text` at current position.
    /// Return `None` when maximum buffer size has been reached,
    /// `true` when the character has been appended to the end of the line.
    pub fn yank(&mut self, text: &str, anchor: Anchor, n: RepeatCount) -> Option<bool> {
        let shift = text.len() * n;
        if text.is_empty() || (self.buf.len() + shift) > self.buf.capacity() {
            return None;
        }
        if let Anchor::After = anchor {
            self.move_forward(1);
        }
        let push = self.pos == self.buf.len();
        if push {
            self.buf.reserve(shift);
            for _ in 0..n {
                self.buf.push_str(text);
            }
        } else {
            let text = iter::repeat(text).take(n).collect::<String>();
            let pos = self.pos;
            self.insert_str(pos, &text);
        }
        self.pos += shift;
        Some(push)
    }

    /// Delete previously yanked text and yank/paste `text` at current position.
    pub fn yank_pop(&mut self, yank_size: usize, text: &str) -> Option<bool> {
        self.buf.drain((self.pos - yank_size)..self.pos);
        self.pos -= yank_size;
        self.yank(text, Anchor::Before, 1)
    }

    /// Move cursor on the left.
    pub fn move_backward(&mut self, n: RepeatCount) -> bool {
        match self.prev_pos(n) {
            Some(pos) => {
                self.pos = pos;
                true
            }
            None => false,
        }
    }

    /// Move cursor on the right.
    pub fn move_forward(&mut self, n: RepeatCount) -> bool {
        match self.next_pos(n) {
            Some(pos) => {
                self.pos = pos;
                true
            }
            None => false,
        }
    }

    /// Move cursor to the start of the line.
    pub fn move_home(&mut self) -> bool {
        if self.pos > 0 {
            self.pos = 0;
            true
        } else {
            false
        }
    }

    /// Move cursor to the end of the line.
    pub fn move_end(&mut self) -> bool {
        if self.pos == self.buf.len() {
            false
        } else {
            self.pos = self.buf.len();
            true
        }
    }

    /// Delete the character at the right of the cursor without altering the cursor
    /// position. Basically this is what happens with the "Delete" keyboard key.
    /// Return the number of characters deleted.
    pub fn delete(&mut self, n: RepeatCount) -> Option<String> {
        match self.next_pos(n) {
            Some(pos) => {
                let chars = self.buf.drain(self.pos..pos).collect::<String>();
                Some(chars)
            }
            None => None,
        }
    }

    /// Delete the character at the left of the cursor.
    /// Basically that is what happens with the "Backspace" keyboard key.
    pub fn backspace(&mut self, n: RepeatCount) -> Option<String> {
        match self.prev_pos(n) {
            Some(pos) => {
                let chars = self.buf.drain(pos..self.pos).collect::<String>();
                self.pos = pos;
                Some(chars)
            }
            None => None,
        }
    }

    /// Kill all characters on the current line.
    pub fn kill_whole_line(&mut self) -> Option<String> {
        self.move_home();
        self.kill_line()
    }

    /// Kill the text from point to the end of the line.
    pub fn kill_line(&mut self) -> Option<String> {
        if !self.buf.is_empty() && self.pos < self.buf.len() {
            let text = self.buf.drain(self.pos..).collect();
            Some(text)
        } else {
            None
        }
    }

    /// Kill backward from point to the beginning of the line.
    pub fn discard_line(&mut self) -> Option<String> {
        if self.pos > 0 && !self.buf.is_empty() {
            let text = self.buf.drain(..self.pos).collect();
            self.pos = 0;
            Some(text)
        } else {
            None
        }
    }

    /// Exchange the char before cursor with the character at cursor.
    pub fn transpose_chars(&mut self) -> bool {
        if self.pos == 0 || self.buf.graphemes(true).count() < 2 {
            return false;
        }
        if self.pos == self.buf.len() {
            self.move_backward(1);
        }
        let chars = self.delete(1).unwrap();
        self.move_backward(1);
        self.yank(&chars, Anchor::Before, 1);
        self.move_forward(1);
        true
    }

    /// Go left until start of word
    fn prev_word_pos(&self, pos: usize, word_def: Word, n: RepeatCount) -> Option<usize> {
        if pos == 0 {
            return None;
        }
        let mut sow = 0;
        let mut gis = self.buf[..pos]
            .grapheme_indices(true)
            .rev();
        'outer: for _ in 0..n {
            let mut gj = gis.next();
            'inner: loop {
                match gj {
                    Some((j, y)) => {
                        let gi = gis.next();
                        match gi {
                            Some((_, x)) => {
                                if is_start_of_word(word_def, x, y) {
                                    sow = j;
                                    break 'inner;
                                }
                                gj = gi;
                            }
                            None => {
                                break 'outer;
                            }
                        }
                    }
                    None => {
                        break 'outer;
                    }
                }
            }
        }
        Some(sow)
    }

    /// Moves the cursor to the beginning of previous word.
    pub fn move_to_prev_word(&mut self, word_def: Word, n: RepeatCount) -> bool {
        if let Some(pos) = self.prev_word_pos(self.pos, word_def, n) {
            self.pos = pos;
            true
        } else {
            false
        }
    }

    /// Delete the previous word, maintaining the cursor at the start of the
    /// current word.
    pub fn delete_prev_word(&mut self, word_def: Word, n: RepeatCount) -> Option<String> {
        if let Some(pos) = self.prev_word_pos(self.pos, word_def, n) {
            let word = self.buf.drain(pos..self.pos).collect();
            self.pos = pos;
            Some(word)
        } else {
            None
        }
    }

    fn next_word_pos(&self, pos: usize, at: At, word_def: Word, n: RepeatCount) -> Option<usize> {
        if pos == self.buf.len() {
            return None;
        }
        let mut wp = self.buf.len() - pos;
        let mut gis = self.buf[pos..].grapheme_indices(true);
        if at == At::End {
            // TODO Validate
            gis.next();
        }
        'outer: for _ in 0..n {
            let mut gi = gis.next();
            'inner: loop {
                match gi {
                    Some((i, x)) => {
                        let gj = gis.next();
                        match gj {
                            Some((j, y)) => {
                                if at == At::Start && is_start_of_word(word_def, x, y) {
                                    wp = j;
                                    break 'inner;
                                } else if at == At::End && is_end_of_word(word_def, x, y) {
                                    if word_def == Word::Emacs {
                                        wp = j;
                                    } else {
                                        wp = i;
                                    }
                                    break 'inner;
                                }
                                gi = gj;
                            }
                            None => {
                                break 'outer;
                            }
                        }
                    }
                    None => {
                        break 'outer;
                    }
                }
            }
        }
        Some(wp + pos)
    }

    /// Moves the cursor to the end of next word.
    pub fn move_to_next_word(&mut self, at: At, word_def: Word, n: RepeatCount) -> bool {
        if let Some(pos) = self.next_word_pos(self.pos, at, word_def, n) {
            self.pos = pos;
            true
        } else {
            false
        }
    }

    fn search_char_pos(&self, cs: &CharSearch, n: RepeatCount) -> Option<usize> {
        let mut shift = 0;
        let search_result = match *cs {
            CharSearch::Backward(c) |
            CharSearch::BackwardAfter(c) => {
                self.buf[..self.pos]
                    .char_indices()
                    .rev()
                    .filter(|&(_, ch)| ch == c)
                    .nth(n - 1)
                    .map(|(i, _)| i)
            }
            CharSearch::Forward(c) |
            CharSearch::ForwardBefore(c) => {
                if let Some(cc) = self.char_at_cursor() {
                    shift = self.pos + cc.len_utf8();
                    if shift < self.buf.len() {
                        self.buf[shift..]
                            .char_indices()
                            .filter(|&(_, ch)| ch == c)
                            .nth(n - 1)
                            .map(|(i, _)| i)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
        };
        if let Some(pos) = search_result {
            Some(match *cs {
                CharSearch::Backward(_) => pos,
                CharSearch::BackwardAfter(c) => pos + c.len_utf8(),
                CharSearch::Forward(_) => shift + pos,
                CharSearch::ForwardBefore(_) => {
                    shift + pos - self.buf[..shift + pos].chars().next_back().unwrap().len_utf8()
                }
            })
        } else {
            None
        }
    }

    pub fn move_to(&mut self, cs: CharSearch, n: RepeatCount) -> bool {
        if let Some(pos) = self.search_char_pos(&cs, n) {
            self.pos = pos;
            true
        } else {
            false
        }
    }

    /// Kill from the cursor to the end of the current word,
    /// or, if between words, to the end of the next word.
    pub fn delete_word(&mut self, at: At, word_def: Word, n: RepeatCount) -> Option<String> {
        if let Some(pos) = self.next_word_pos(self.pos, at, word_def, n) {
            let word = self.buf.drain(self.pos..pos).collect();
            Some(word)
        } else {
            None
        }
    }

    pub fn delete_to(&mut self, cs: CharSearch, n: RepeatCount) -> Option<String> {
        let search_result = match cs {
            CharSearch::ForwardBefore(c) => self.search_char_pos(&CharSearch::Forward(c), n),
            _ => self.search_char_pos(&cs, n),
        };
        if let Some(pos) = search_result {
            let chunk = match cs {
                CharSearch::Backward(_) |
                CharSearch::BackwardAfter(_) => {
                    let end = self.pos;
                    self.pos = pos;
                    self.buf.drain(pos..end).collect()
                }
                CharSearch::ForwardBefore(_) => self.buf.drain(self.pos..pos).collect(),
                CharSearch::Forward(c) => self.buf.drain(self.pos..pos + c.len_utf8()).collect(),
            };
            Some(chunk)
        } else {
            None
        }
    }

    /// Alter the next word.
    pub fn edit_word(&mut self, a: WordAction) -> bool {
        if let Some(start) = self.next_word_pos(self.pos, At::Start, Word::Emacs, 1) {
            if let Some(end) = self.next_word_pos(self.pos, At::End, Word::Emacs, 1) {
                if start == end {
                    return false;
                }
                let word = self.buf.drain(start..end).collect::<String>();
                let result = match a {
                    WordAction::CAPITALIZE => {
                        let ch = (&word).graphemes(true).next().unwrap();
                        let cap = ch.to_uppercase();
                        cap + &word[ch.len()..].to_lowercase()
                    }
                    WordAction::LOWERCASE => word.to_lowercase(),
                    WordAction::UPPERCASE => word.to_uppercase(),
                };
                self.insert_str(start, &result);
                self.pos = start + result.len();
                return true;
            }
        }
        false
    }

    /// Transpose two words
    pub fn transpose_words(&mut self, n: RepeatCount) -> bool {
        let word_def = Word::Emacs;
        self.move_to_next_word(At::End, word_def, n);
        let w2_end = self.pos;
        self.move_to_prev_word(word_def, 1);
        let w2_beg = self.pos;
        self.move_to_prev_word(word_def, n);
        let w1_beg = self.pos;
        self.move_to_next_word(At::End, word_def, 1);
        let w1_end = self.pos;
        if w1_beg == w2_beg || w2_beg < w1_end {
            return false;
        }

        let w1 = self.buf[w1_beg..w1_end].to_string();

        let w2 = self.buf.drain(w2_beg..w2_end).collect::<String>();
        self.insert_str(w2_beg, &w1);

        self.buf.drain(w1_beg..w1_end);
        self.insert_str(w1_beg, &w2);

        self.pos = w2_end;
        true
    }

    /// Replaces the content between [`start`..`end`] with `text`
    /// and positions the cursor to the end of text.
    pub fn replace(&mut self, range: Range<usize>, text: &str) {
        let start = range.start;
        self.buf.drain(range);
        self.insert_str(start, text);
        self.pos = start + text.len();
    }

    fn insert_str(&mut self, idx: usize, s: &str) -> bool {
        if idx == self.buf.len() {
            self.buf.push_str(s);
            true
        } else {
            self.buf.insert_str(idx, s);
            false
        }
    }

    pub fn copy(&self, mvt: Movement) -> Option<String> {
        if self.is_empty() {
            return None;
        }
        match mvt {
            Movement::WholeLine => Some(self.buf.clone()),
            Movement::BeginningOfLine => {
                if self.pos == 0 {
                    None
                } else {
                    Some(self.buf[..self.pos].to_string())
                }
            }
            Movement::EndOfLine => {
                if self.pos == self.buf.len() {
                    None
                } else {
                    Some(self.buf[self.pos..].to_string())
                }
            }
            Movement::BackwardWord(n, word_def) => {
                if let Some(pos) = self.prev_word_pos(self.pos, word_def, n) {
                    Some(self.buf[pos..self.pos].to_string())
                } else {
                    None
                }
            }
            Movement::ForwardWord(n, at, word_def) => {
                if let Some(pos) = self.next_word_pos(self.pos, at, word_def, n) {
                    Some(self.buf[self.pos..pos].to_string())
                } else {
                    None
                }
            }
            Movement::ViCharSearch(n, cs) => {
                let search_result = match cs {
                    CharSearch::ForwardBefore(c) => {
                        self.search_char_pos(&CharSearch::Forward(c), n)
                    }
                    _ => self.search_char_pos(&cs, n),
                };
                if let Some(pos) = search_result {
                    Some(match cs {
                        CharSearch::Backward(_) |
                        CharSearch::BackwardAfter(_) => self.buf[pos..self.pos].to_string(),
                        CharSearch::ForwardBefore(_) => self.buf[self.pos..pos].to_string(),
                        CharSearch::Forward(c) => {
                            self.buf[self.pos..pos + c.len_utf8()].to_string()
                        }
                    })
                } else {
                    None
                }
            }
            Movement::BackwardChar(n) => {
                if let Some(pos) = self.prev_pos(n) {
                    Some(self.buf[pos..self.pos].to_string())
                } else {
                    None
                }
            }
            Movement::ForwardChar(n) => {
                if let Some(pos) = self.next_pos(n) {
                    Some(self.buf[self.pos..pos].to_string())
                } else {
                    None
                }
            }
        }
    }
}

impl Deref for LineBuffer {
    type Target = str;

    fn deref(&self) -> &str {
        self.as_str()
    }
}

fn is_start_of_word(word_def: Word, previous: &str, grapheme: &str) -> bool {
    (!is_word_char(word_def, previous) && is_word_char(word_def, grapheme)) ||
    (word_def == Word::Vi && !is_other_char(previous) && is_other_char(grapheme))
}
fn is_end_of_word(word_def: Word, grapheme: &str, next: &str) -> bool {
    (!is_word_char(word_def, next) && is_word_char(word_def, grapheme)) ||
    (word_def == Word::Vi && !is_other_char(next) && is_other_char(grapheme))
}

fn is_word_char(word_def: Word, grapheme: &str) -> bool {
    match word_def {
        Word::Emacs => grapheme.is_alphanumeric(),
        Word::Vi => is_vi_word_char(grapheme),
        Word::Big => !grapheme.is_whitespace(),
    }
}
fn is_vi_word_char(grapheme: &str) -> bool {
    grapheme.is_alphanumeric() || grapheme == "_"
}
fn is_other_char(grapheme: &str) -> bool {
    !(grapheme.is_whitespace() || is_vi_word_char(grapheme))
}

#[cfg(test)]
mod test {
    use keymap::{Anchor, At, CharSearch, Word};
    use super::{LineBuffer, MAX_LINE, WordAction};

    #[test]
    fn next_pos() {
        let s = LineBuffer::init("ö̲g̈", 0);
        assert_eq!(7, s.len());
        let pos = s.next_pos(1);
        assert_eq!(Some(4), pos);

        let s = LineBuffer::init("ö̲g̈", 4);
        let pos = s.next_pos(1);
        assert_eq!(Some(7), pos);
    }

    #[test]
    fn prev_pos() {
        let s = LineBuffer::init("ö̲g̈", 4);
        assert_eq!(7, s.len());
        let pos = s.prev_pos(1);
        assert_eq!(Some(0), pos);

        let s = LineBuffer::init("ö̲g̈", 7);
        let pos = s.prev_pos(1);
        assert_eq!(Some(4), pos);
    }

    #[test]
    fn insert() {
        let mut s = LineBuffer::with_capacity(MAX_LINE);
        let push = s.insert('α', 1).unwrap();
        assert_eq!("α", s.buf);
        assert_eq!(2, s.pos);
        assert_eq!(true, push);

        let push = s.insert('ß', 1).unwrap();
        assert_eq!("αß", s.buf);
        assert_eq!(4, s.pos);
        assert_eq!(true, push);

        s.pos = 0;
        let push = s.insert('γ', 1).unwrap();
        assert_eq!("γαß", s.buf);
        assert_eq!(2, s.pos);
        assert_eq!(false, push);
    }

    #[test]
    fn yank_after() {
        let mut s = LineBuffer::init("αß", 2);
        let ok = s.yank("γδε", Anchor::After, 1);
        assert_eq!(Some(true), ok);
        assert_eq!("αßγδε", s.buf);
        assert_eq!(10, s.pos);
    }

    #[test]
    fn yank_before() {
        let mut s = LineBuffer::init("αε", 2);
        let ok = s.yank("ßγδ", Anchor::Before, 1);
        assert_eq!(Some(false), ok);
        assert_eq!("αßγδε", s.buf);
        assert_eq!(8, s.pos);
    }

    #[test]
    fn moves() {
        let mut s = LineBuffer::init("αß", 4);
        let ok = s.move_backward(1);
        assert_eq!("αß", s.buf);
        assert_eq!(2, s.pos);
        assert_eq!(true, ok);

        let ok = s.move_forward(1);
        assert_eq!("αß", s.buf);
        assert_eq!(4, s.pos);
        assert_eq!(true, ok);

        let ok = s.move_home();
        assert_eq!("αß", s.buf);
        assert_eq!(0, s.pos);
        assert_eq!(true, ok);

        let ok = s.move_end();
        assert_eq!("αß", s.buf);
        assert_eq!(4, s.pos);
        assert_eq!(true, ok);
    }

    #[test]
    fn move_grapheme() {
        let mut s = LineBuffer::init("ag̈", 4);
        assert_eq!(4, s.len());
        let ok = s.move_backward(1);
        assert_eq!(true, ok);
        assert_eq!(1, s.pos);

        let ok = s.move_forward(1);
        assert_eq!(true, ok);
        assert_eq!(4, s.pos);
    }

    #[test]
    fn delete() {
        let mut s = LineBuffer::init("αß", 2);
        let chars = s.delete(1);
        assert_eq!("α", s.buf);
        assert_eq!(2, s.pos);
        assert_eq!(Some("ß".to_string()), chars);

        let chars = s.backspace(1);
        assert_eq!("", s.buf);
        assert_eq!(0, s.pos);
        assert_eq!(Some("α".to_string()), chars);
    }

    #[test]
    fn kill() {
        let mut s = LineBuffer::init("αßγδε", 6);
        let text = s.kill_line();
        assert_eq!("αßγ", s.buf);
        assert_eq!(6, s.pos);
        assert_eq!(Some("δε".to_string()), text);

        s.pos = 4;
        let text = s.discard_line();
        assert_eq!("γ", s.buf);
        assert_eq!(0, s.pos);
        assert_eq!(Some("αß".to_string()), text);
    }

    #[test]
    fn transpose() {
        let mut s = LineBuffer::init("aßc", 1);
        let ok = s.transpose_chars();
        assert_eq!("ßac", s.buf);
        assert_eq!(3, s.pos);
        assert_eq!(true, ok);

        s.buf = String::from("aßc");
        s.pos = 3;
        let ok = s.transpose_chars();
        assert_eq!("acß", s.buf);
        assert_eq!(4, s.pos);
        assert_eq!(true, ok);

        s.buf = String::from("aßc");
        s.pos = 4;
        let ok = s.transpose_chars();
        assert_eq!("acß", s.buf);
        assert_eq!(4, s.pos);
        assert_eq!(true, ok);
    }

    #[test]
    fn move_to_prev_word() {
        let mut s = LineBuffer::init("a ß  c", 6);
        let ok = s.move_to_prev_word(Word::Emacs, 1);
        assert_eq!("a ß  c", s.buf);
        assert_eq!(2, s.pos);
        assert_eq!(true, ok);
    }

    #[test]
    fn move_to_prev_vi_word() {
        let mut s = LineBuffer::init("alpha ,beta/rho; mu", 19);
        let ok = s.move_to_prev_word(Word::Vi, 1);
        assert!(ok);
        assert_eq!(17, s.pos);
        let ok = s.move_to_prev_word(Word::Vi, 1);
        assert!(ok);
        assert_eq!(15, s.pos);
        let ok = s.move_to_prev_word(Word::Vi, 1);
        assert!(ok);
        assert_eq!(12, s.pos);
        let ok = s.move_to_prev_word(Word::Vi, 1);
        assert!(ok);
        assert_eq!(11, s.pos);
        let ok = s.move_to_prev_word(Word::Vi, 1);
        assert!(ok);
        assert_eq!(7, s.pos);
        let ok = s.move_to_prev_word(Word::Vi, 1);
        assert!(ok);
        assert_eq!(6, s.pos);
        let ok = s.move_to_prev_word(Word::Vi, 1);
        assert!(ok);
        assert_eq!(0, s.pos);
        let ok = s.move_to_prev_word(Word::Vi, 1);
        assert!(!ok);
    }

    #[test]
    fn move_to_prev_big_word() {
        let mut s = LineBuffer::init("alpha ,beta/rho; mu", 19);
        let ok = s.move_to_prev_word(Word::Big, 1);
        assert!(ok);
        assert_eq!(17, s.pos);
        let ok = s.move_to_prev_word(Word::Big, 1);
        assert!(ok);
        assert_eq!(6, s.pos);
        let ok = s.move_to_prev_word(Word::Big, 1);
        assert!(ok);
        assert_eq!(0, s.pos);
        let ok = s.move_to_prev_word(Word::Big, 1);
        assert!(!ok);
    }

    #[test]
    fn move_to_forward() {
        let mut s = LineBuffer::init("αßγδε", 2);
        let ok = s.move_to(CharSearch::ForwardBefore('ε'), 1);
        assert_eq!(true, ok);
        assert_eq!(6, s.pos);

        let mut s = LineBuffer::init("αßγδε", 2);
        let ok = s.move_to(CharSearch::Forward('ε'), 1);
        assert_eq!(true, ok);
        assert_eq!(8, s.pos);
    }

    #[test]
    fn move_to_backward() {
        let mut s = LineBuffer::init("αßγδε", 8);
        let ok = s.move_to(CharSearch::BackwardAfter('ß'), 1);
        assert_eq!(true, ok);
        assert_eq!(4, s.pos);

        let mut s = LineBuffer::init("αßγδε", 8);
        let ok = s.move_to(CharSearch::Backward('ß'), 1);
        assert_eq!(true, ok);
        assert_eq!(2, s.pos);
    }

    #[test]
    fn delete_prev_word() {
        let mut s = LineBuffer::init("a ß  c", 6);
        let text = s.delete_prev_word(Word::Big, 1);
        assert_eq!("a c", s.buf);
        assert_eq!(2, s.pos);
        assert_eq!(Some("ß  ".to_string()), text);
    }

    #[test]
    fn move_to_next_word() {
        let mut s = LineBuffer::init("a ß  c", 1);
        let ok = s.move_to_next_word(At::End, Word::Emacs, 1);
        assert_eq!("a ß  c", s.buf);
        assert_eq!(4, s.pos);
        assert_eq!(true, ok);
    }

    #[test]
    fn move_to_end_of_word() {
        let mut s = LineBuffer::init("a ßeta  c", 1);
        let ok = s.move_to_next_word(At::End, Word::Vi, 1);
        assert_eq!("a ßeta  c", s.buf);
        assert_eq!(6, s.pos);
        assert_eq!(true, ok);
    }

    #[test]
    fn move_to_end_of_vi_word() {
        let mut s = LineBuffer::init("alpha ,beta/rho; mu", 0);
        let ok = s.move_to_next_word(At::End, Word::Vi, 1);
        assert!(ok);
        assert_eq!(4, s.pos);
        let ok = s.move_to_next_word(At::End, Word::Vi, 1);
        assert!(ok);
        assert_eq!(6, s.pos);
        let ok = s.move_to_next_word(At::End, Word::Vi, 1);
        assert!(ok);
        assert_eq!(10, s.pos);
        let ok = s.move_to_next_word(At::End, Word::Vi, 1);
        assert!(ok);
        assert_eq!(11, s.pos);
        let ok = s.move_to_next_word(At::End, Word::Vi, 1);
        assert!(ok);
        assert_eq!(14, s.pos);
        let ok = s.move_to_next_word(At::End, Word::Vi, 1);
        assert!(ok);
        assert_eq!(15, s.pos);
        let ok = s.move_to_next_word(At::End, Word::Vi, 1);
        assert!(ok);
        assert_eq!(19, s.pos);
        let ok = s.move_to_next_word(At::End, Word::Vi, 1);
        assert!(!ok);
    }

    #[test]
    fn move_to_end_of_big_word() {
        let mut s = LineBuffer::init("alpha ,beta/rho; mu", 0);
        let ok = s.move_to_next_word(At::End, Word::Big, 1);
        assert!(ok);
        assert_eq!(4, s.pos);
        let ok = s.move_to_next_word(At::End, Word::Big, 1);
        assert!(ok);
        assert_eq!(15, s.pos);
        let ok = s.move_to_next_word(At::End, Word::Big, 1);
        assert!(ok);
        assert_eq!(19, s.pos);
        let ok = s.move_to_next_word(At::End, Word::Big, 1);
        assert!(!ok);
    }

    #[test]
    fn move_to_start_of_word() {
        let mut s = LineBuffer::init("a ß  c", 2);
        let ok = s.move_to_next_word(At::Start, Word::Emacs, 1);
        assert_eq!("a ß  c", s.buf);
        assert_eq!(6, s.pos);
        assert_eq!(true, ok);
    }

    #[test]
    fn move_to_start_of_vi_word() {
        let mut s = LineBuffer::init("alpha ,beta/rho; mu", 0);
        let ok = s.move_to_next_word(At::Start, Word::Vi, 1);
        assert!(ok);
        assert_eq!(6, s.pos);
        let ok = s.move_to_next_word(At::Start, Word::Vi, 1);
        assert!(ok);
        assert_eq!(7, s.pos);
        let ok = s.move_to_next_word(At::Start, Word::Vi, 1);
        assert!(ok);
        assert_eq!(11, s.pos);
        let ok = s.move_to_next_word(At::Start, Word::Vi, 1);
        assert!(ok);
        assert_eq!(12, s.pos);
        let ok = s.move_to_next_word(At::Start, Word::Vi, 1);
        assert!(ok);
        assert_eq!(15, s.pos);
        let ok = s.move_to_next_word(At::Start, Word::Vi, 1);
        assert!(ok);
        assert_eq!(17, s.pos);
        let ok = s.move_to_next_word(At::Start, Word::Vi, 1);
        assert!(ok);
        assert_eq!(19, s.pos);
        let ok = s.move_to_next_word(At::Start, Word::Vi, 1);
        assert!(!ok);
    }

    #[test]
    fn move_to_start_of_big_word() {
        let mut s = LineBuffer::init("alpha ,beta/rho; mu", 0);
        let ok = s.move_to_next_word(At::Start, Word::Big, 1);
        assert!(ok);
        assert_eq!(6, s.pos);
        let ok = s.move_to_next_word(At::Start, Word::Big, 1);
        assert!(ok);
        assert_eq!(17, s.pos);
        let ok = s.move_to_next_word(At::Start, Word::Big, 1);
        assert!(ok);
        assert_eq!(19, s.pos);
        let ok = s.move_to_next_word(At::Start, Word::Big, 1);
        assert!(!ok);
    }

    #[test]
    fn delete_word() {
        let mut s = LineBuffer::init("a ß  c", 1);
        let text = s.delete_word(At::End, Word::Emacs, 1);
        assert_eq!("a  c", s.buf);
        assert_eq!(1, s.pos);
        assert_eq!(Some(" ß".to_string()), text);
    }

    #[test]
    fn delete_til_start_of_word() {
        let mut s = LineBuffer::init("a ß  c", 2);
        let text = s.delete_word(At::Start, Word::Emacs, 1);
        assert_eq!("a c", s.buf);
        assert_eq!(2, s.pos);
        assert_eq!(Some("ß  ".to_string()), text);
    }

    #[test]
    fn delete_to_forward() {
        let mut s = LineBuffer::init("αßγδε", 2);
        let text = s.delete_to(CharSearch::ForwardBefore('ε'), 1);
        assert_eq!(Some("ßγδ".to_string()), text);
        assert_eq!("αε", s.buf);
        assert_eq!(2, s.pos);

        let mut s = LineBuffer::init("αßγδε", 2);
        let text = s.delete_to(CharSearch::Forward('ε'), 1);
        assert_eq!(Some("ßγδε".to_string()), text);
        assert_eq!("α", s.buf);
        assert_eq!(2, s.pos);
    }

    #[test]
    fn delete_to_backward() {
        let mut s = LineBuffer::init("αßγδε", 8);
        let text = s.delete_to(CharSearch::BackwardAfter('α'), 1);
        assert_eq!(Some("ßγδ".to_string()), text);
        assert_eq!("αε", s.buf);
        assert_eq!(2, s.pos);

        let mut s = LineBuffer::init("αßγδε", 8);
        let text = s.delete_to(CharSearch::Backward('ß'), 1);
        assert_eq!(Some("ßγδ".to_string()), text);
        assert_eq!("αε", s.buf);
        assert_eq!(2, s.pos);
    }

    #[test]
    fn edit_word() {
        let mut s = LineBuffer::init("a ßeta  c", 1);
        assert!(s.edit_word(WordAction::UPPERCASE));
        assert_eq!("a SSETA  c", s.buf);
        assert_eq!(7, s.pos);

        let mut s = LineBuffer::init("a ßetA  c", 1);
        assert!(s.edit_word(WordAction::LOWERCASE));
        assert_eq!("a ßeta  c", s.buf);
        assert_eq!(7, s.pos);

        let mut s = LineBuffer::init("a ßETA  c", 1);
        assert!(s.edit_word(WordAction::CAPITALIZE));
        assert_eq!("a SSeta  c", s.buf);
        assert_eq!(7, s.pos);
    }

    #[test]
    fn transpose_words() {
        let mut s = LineBuffer::init("ßeta / δelta__", 15);
        assert!(s.transpose_words(1));
        assert_eq!("δelta__ / ßeta", s.buf);
        assert_eq!(16, s.pos);

        let mut s = LineBuffer::init("ßeta / δelta", 14);
        assert!(s.transpose_words(1));
        assert_eq!("δelta / ßeta", s.buf);
        assert_eq!(14, s.pos);

        let mut s = LineBuffer::init(" / δelta", 8);
        assert!(!s.transpose_words(1));

        let mut s = LineBuffer::init("ßeta / __", 9);
        assert!(!s.transpose_words(1));
    }
}
