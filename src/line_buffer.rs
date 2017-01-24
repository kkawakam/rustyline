//! Line buffer with current cursor position
use std::iter;
use std::ops::{Deref, Range};
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
            self.move_right(1);
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
    pub fn move_left(&mut self, n: RepeatCount) -> bool {
        match self.prev_pos(n) {
            Some(pos) => {
                self.pos = pos;
                true
            }
            None => false,
        }
    }

    /// Move cursor on the right.
    pub fn move_right(&mut self, n: RepeatCount) -> bool {
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
        if self.pos == 0 || self.buf.chars().count() < 2 {
            return false;
        }
        if self.pos == self.buf.len() {
            self.move_left(1);
        }
        let chars = self.delete(1).unwrap();
        self.move_left(1);
        self.yank(&chars, Anchor::Before, 1);
        self.move_right(1);
        true
    }

    /// Go left until start of word
    fn prev_word_pos(&self, pos: usize, word_def: Word, n: RepeatCount) -> Option<usize> {
        if pos == 0 {
            return None;
        }
        let test = is_break_char(word_def);
        let mut pos = pos;
        for _ in 0..n {
            // eat any spaces on the left
            pos -= self.buf[..pos]
                .chars()
                .rev()
                .take_while(|ch| test(ch))
                .map(char::len_utf8)
                .sum();
            if pos > 0 {
                // eat any non-spaces on the left
                pos -= self.buf[..pos]
                    .chars()
                    .rev()
                    .take_while(|ch| !test(ch))
                    .map(char::len_utf8)
                    .sum();
            }
            if pos == 0 {
                break;
            }
        }
        Some(pos)
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
        match at {
            At::End => {
                match self.next_end_of_word_pos(pos, word_def, n) {
                    Some((_, end)) => Some(end),
                    _ => None,
                }
            }
            At::Start => self.next_start_of_word_pos(pos, word_def, n),
        }
    }

    /// Go right until start of word
    fn next_start_of_word_pos(&self, pos: usize, word_def: Word, n: RepeatCount) -> Option<usize> {
        if pos == self.buf.len() {
            return None;
        }
        let test = is_break_char(word_def);
        let mut pos = pos;
        for _ in 0..n {
            // eat any non-spaces
            pos += self.buf[pos..]
                .chars()
                .take_while(|ch| !test(ch))
                .map(char::len_utf8)
                .sum();
            if pos < self.buf.len() {
                // eat any spaces
                pos += self.buf[pos..]
                    .chars()
                    .take_while(test)
                    .map(char::len_utf8)
                    .sum();
            }
            if pos == self.buf.len() {
                break;
            }
        }
        Some(pos)
    }

    /// Go right until end of word
    /// Returns the position (start, end) of the next word.
    fn next_end_of_word_pos(&self,
                            pos: usize,
                            word_def: Word,
                            n: RepeatCount)
                            -> Option<(usize, usize)> {
        if pos == self.buf.len() {
            return None;
        }
        let test = is_break_char(word_def);
        let mut pos = pos;
        let mut start = pos;
        for _ in 0..n {
            // eat any spaces
            pos += self.buf[pos..]
                .chars()
                .take_while(test)
                .map(char::len_utf8)
                .sum();
            start = pos;
            if pos < self.buf.len() {
                // eat any non-spaces
                pos += self.buf[pos..]
                    .chars()
                    .take_while(|ch| !test(ch))
                    .map(char::len_utf8)
                    .sum();
            }
            if pos == self.buf.len() {
                break;
            }
        }
        Some((start, pos))
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

    /// Kill from the cursor to the end of the current word, or, if between words, to the end of the next word.
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
        if let Some((start, end)) = self.next_end_of_word_pos(self.pos, Word::Emacs, 1) {
            if start == end {
                return false;
            }
            let word = self.buf.drain(start..end).collect::<String>();
            let result = match a {
                WordAction::CAPITALIZE => {
                    if let Some(ch) = word.chars().next() {
                        let cap = ch.to_uppercase().collect::<String>();
                        cap + &word[ch.len_utf8()..].to_lowercase()
                    } else {
                        word
                    }
                }
                WordAction::LOWERCASE => word.to_lowercase(),
                WordAction::UPPERCASE => word.to_uppercase(),
            };
            self.insert_str(start, &result);
            self.pos = start + result.len();
            true
        } else {
            false
        }
    }

    /// Transpose two words
    pub fn transpose_words(&mut self) -> bool {
        // prevword___oneword__
        // ^          ^       ^
        // prev_start start   self.pos/end
        let word_def = Word::Emacs;
        if let Some(start) = self.prev_word_pos(self.pos, word_def, 1) {
            if let Some(prev_start) = self.prev_word_pos(start, word_def, 1) {
                let (_, prev_end) = self.next_end_of_word_pos(prev_start, word_def, 1).unwrap();
                if prev_end >= start {
                    return false;
                }
                let (_, mut end) = self.next_end_of_word_pos(start, word_def, 1).unwrap();
                if end < self.pos {
                    if self.pos < self.buf.len() {
                        let (s, _) = self.next_end_of_word_pos(self.pos, word_def, 1).unwrap();
                        end = s;
                    } else {
                        end = self.pos;
                    }
                }

                let oneword = self.buf.drain(start..end).collect::<String>();
                let sep = self.buf.drain(prev_end..start).collect::<String>();
                let prevword = self.buf.drain(prev_start..prev_end).collect::<String>();

                let mut idx = prev_start;
                self.insert_str(idx, &oneword);
                idx += oneword.len();
                self.insert_str(idx, &sep);
                idx += sep.len();
                self.insert_str(idx, &prevword);

                self.pos = idx + prevword.len();
                return true;
            }
        }
        false
    }

    /// Replaces the content between [`start`..`end`] with `text` and positions the cursor to the end of text.
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
            insert_str(&mut self.buf, idx, s);
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
                let search_result = self.search_char_pos(&cs, n);
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

fn insert_str(buf: &mut String, idx: usize, s: &str) {
    use std::ptr;

    let len = buf.len();
    assert!(idx <= len);
    assert!(buf.is_char_boundary(idx));
    let amt = s.len();
    buf.reserve(amt);

    unsafe {
        let v = buf.as_mut_vec();
        ptr::copy(v.as_ptr().offset(idx as isize),
                  v.as_mut_ptr().offset((idx + amt) as isize),
                  len - idx);
        ptr::copy_nonoverlapping(s.as_ptr(), v.as_mut_ptr().offset(idx as isize), amt);
        v.set_len(len + amt);
    }
}

fn is_break_char(word_def: Word) -> fn(&char) -> bool {
    match word_def {
        Word::Emacs => is_not_alphanumeric,
        Word::Vi => is_not_alphanumeric_and_underscore,
        Word::Big => is_whitespace,
    }
}

fn is_not_alphanumeric(ch: &char) -> bool {
    !ch.is_alphanumeric()
}
fn is_not_alphanumeric_and_underscore(ch: &char) -> bool {
    !ch.is_alphanumeric() && *ch != '_'
}
fn is_whitespace(ch: &char) -> bool {
    ch.is_whitespace()
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
        let ok = s.move_left(1);
        assert_eq!("αß", s.buf);
        assert_eq!(2, s.pos);
        assert_eq!(true, ok);

        let ok = s.move_right(1);
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
        let ok = s.move_left(1);
        assert_eq!(true, ok);
        assert_eq!(1, s.pos);

        let ok = s.move_right(1);
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
    fn move_to_start_of_word() {
        let mut s = LineBuffer::init("a ß  c", 2);
        let ok = s.move_to_next_word(At::Start, Word::Emacs, 1);
        assert_eq!("a ß  c", s.buf);
        assert_eq!(6, s.pos);
        assert_eq!(true, ok);
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
        assert!(s.transpose_words());
        assert_eq!("δelta__ / ßeta", s.buf);
        assert_eq!(16, s.pos);

        let mut s = LineBuffer::init("ßeta / δelta", 14);
        assert!(s.transpose_words());
        assert_eq!("δelta / ßeta", s.buf);
        assert_eq!(14, s.pos);

        let mut s = LineBuffer::init(" / δelta", 8);
        assert!(!s.transpose_words());

        let mut s = LineBuffer::init("ßeta / __", 9);
        assert!(!s.transpose_words());
    }
}
