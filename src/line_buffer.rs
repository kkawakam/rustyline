//! Line buffer with current cursor position
use keymap::{At, CharSearch, Movement, RepeatCount, Word};
use std::cell::RefCell;
use std::fmt;
use std::iter;
use std::ops::{Deref, Index, Range};
use std::rc::Rc;
use std::string::Drain;
use std::sync::{Arc, Mutex};
use unicode_segmentation::UnicodeSegmentation;

/// Maximum buffer size for the line read
pub(crate) static MAX_LINE: usize = 4096;

/// Word's case change
#[derive(Clone, Copy)]
pub enum WordAction {
    CAPITALIZE,
    LOWERCASE,
    UPPERCASE,
}

/// Delete (kill) direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Direction {
    Forward,
    Backward,
}

impl Default for Direction {
    fn default() -> Direction {
        Direction::Forward
    }
}

/// Listener to be notified when some text is deleted.
pub(crate) trait DeleteListener {
    fn start_killing(&mut self);
    fn delete(&mut self, idx: usize, string: &str, dir: Direction);
    fn stop_killing(&mut self);
}

/// Listener to be notified when the line is modified.
pub(crate) trait ChangeListener: DeleteListener {
    fn insert_char(&mut self, idx: usize, c: char);
    fn insert_str(&mut self, idx: usize, string: &str);
    fn replace(&mut self, idx: usize, old: &str, new: &str);
}

/// Represent the current input (text and cursor position).
///
/// The methods do text manipulations or/and cursor movements.
pub struct LineBuffer {
    buf: String, // Edited line buffer (rl_line_buffer)
    pos: usize,  // Current cursor position (byte position) (rl_point)
    dl: Option<Arc<Mutex<DeleteListener>>>,
    cl: Option<Rc<RefCell<ChangeListener>>>,
}

impl fmt::Debug for LineBuffer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("LineBuffer")
            .field("buf", &self.buf)
            .field("pos", &self.pos)
            .finish()
    }
}

impl LineBuffer {
    /// Create a new line buffer with the given maximum `capacity`.
    pub fn with_capacity(capacity: usize) -> LineBuffer {
        LineBuffer {
            buf: String::with_capacity(capacity),
            pos: 0,
            dl: None,
            cl: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn init(
        line: &str,
        pos: usize,
        cl: Option<Rc<RefCell<ChangeListener>>>,
    ) -> LineBuffer {
        let mut lb = Self::with_capacity(MAX_LINE);
        assert!(lb.insert_str(0, line));
        lb.set_pos(pos);
        lb.cl = cl;
        lb
    }

    pub(crate) fn set_delete_listener(&mut self, dl: Arc<Mutex<DeleteListener>>) {
        self.dl = Some(dl);
    }

    pub(crate) fn set_change_listener(&mut self, dl: Rc<RefCell<ChangeListener>>) {
        self.cl = Some(dl);
    }

    pub(crate) fn remove_change_listener(&mut self) {
        self.cl = None;
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

    /// Set cursor position (byte position)
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
        let end = self.len();
        self.drain(0..end, Direction::default());
        let max = self.buf.capacity();
        if buf.len() > max {
            self.insert_str(0, &buf[..max]);
            if pos > max {
                self.pos = max;
            } else {
                self.pos = pos;
            }
        } else {
            self.insert_str(0, buf);
            self.pos = pos;
        }
    }

    /// Returns the character at current cursor position.
    pub(crate) fn grapheme_at_cursor(&self) -> Option<&str> {
        if self.pos == self.buf.len() {
            None
        } else {
            self.buf[self.pos..].graphemes(true).next()
        }
    }

    /// Returns the position of the character just after the current cursor
    /// position.
    pub fn next_pos(&self, n: RepeatCount) -> Option<usize> {
        if self.pos == self.buf.len() {
            return None;
        }
        self.buf[self.pos..]
            .grapheme_indices(true)
            .take(n)
            .last()
            .map(|(i, s)| i + self.pos + s.len())
    }

    /// Returns the position of the character just before the current cursor
    /// position.
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
        if n == 1 {
            self.buf.insert(self.pos, ch);
            for cl in &self.cl {
                cl.borrow_mut().insert_char(self.pos, ch);
            }
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
    pub fn yank(&mut self, text: &str, n: RepeatCount) -> Option<bool> {
        let shift = text.len() * n;
        if text.is_empty() || (self.buf.len() + shift) > self.buf.capacity() {
            return None;
        }
        let push = self.pos == self.buf.len();
        let pos = self.pos;
        if n == 1 {
            self.insert_str(pos, text);
        } else {
            let text = iter::repeat(text).take(n).collect::<String>();
            self.insert_str(pos, &text);
        }
        self.pos += shift;
        Some(push)
    }

    /// Delete previously yanked text and yank/paste `text` at current position.
    pub fn yank_pop(&mut self, yank_size: usize, text: &str) -> Option<bool> {
        let end = self.pos;
        let start = end - yank_size;
        self.drain(start..end, Direction::default());
        self.pos -= yank_size;
        self.yank(text, 1)
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

    /// Delete the character at the right of the cursor without altering the
    /// cursor position. Basically this is what happens with the "Delete"
    /// keyboard key.
    /// Return the number of characters deleted.
    pub fn delete(&mut self, n: RepeatCount) -> Option<String> {
        match self.next_pos(n) {
            Some(pos) => {
                let start = self.pos;
                let chars = self
                    .drain(start..pos, Direction::Forward)
                    .collect::<String>();
                Some(chars)
            }
            None => None,
        }
    }

    /// Delete the character at the left of the cursor.
    /// Basically that is what happens with the "Backspace" keyboard key.
    pub fn backspace(&mut self, n: RepeatCount) -> bool {
        match self.prev_pos(n) {
            Some(pos) => {
                let end = self.pos;
                self.drain(pos..end, Direction::Backward);
                self.pos = pos;
                true
            }
            None => false,
        }
    }

    /// Kill the text from point to the end of the line.
    pub fn kill_line(&mut self) -> bool {
        if !self.buf.is_empty() && self.pos < self.buf.len() {
            let start = self.pos;
            let end = self.buf.len();
            self.drain(start..end, Direction::Forward);
            true
        } else {
            false
        }
    }

    /// Kill backward from point to the beginning of the line.
    pub fn discard_line(&mut self) -> bool {
        if self.pos > 0 && !self.buf.is_empty() {
            let end = self.pos;
            self.drain(0..end, Direction::Backward);
            self.pos = 0;
            true
        } else {
            false
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
        self.yank(&chars, 1);
        self.move_forward(1);
        true
    }

    /// Go left until start of word
    fn prev_word_pos(&self, pos: usize, word_def: Word, n: RepeatCount) -> Option<usize> {
        if pos == 0 {
            return None;
        }
        let mut sow = 0;
        let mut gis = self.buf[..pos].grapheme_indices(true).rev();
        'outer: for _ in 0..n {
            sow = 0;
            let mut gj = gis.next();
            'inner: loop {
                if let Some((j, y)) = gj {
                    let gi = gis.next();
                    if let Some((_, x)) = gi {
                        if is_start_of_word(word_def, x, y) {
                            sow = j;
                            break 'inner;
                        }
                        gj = gi;
                    } else {
                        break 'outer;
                    }
                } else {
                    break 'outer;
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
    pub fn delete_prev_word(&mut self, word_def: Word, n: RepeatCount) -> bool {
        if let Some(pos) = self.prev_word_pos(self.pos, word_def, n) {
            let end = self.pos;
            self.drain(pos..end, Direction::Backward);
            self.pos = pos;
            true
        } else {
            false
        }
    }

    fn next_word_pos(&self, pos: usize, at: At, word_def: Word, n: RepeatCount) -> Option<usize> {
        if pos == self.buf.len() {
            return None;
        }
        let mut wp = 0;
        let mut gis = self.buf[pos..].grapheme_indices(true);
        let mut gi = if at == At::BeforeEnd {
            // TODO Validate
            gis.next()
        } else {
            None
        };
        'outer: for _ in 0..n {
            wp = 0;
            gi = gis.next();
            'inner: loop {
                if let Some((i, x)) = gi {
                    let gj = gis.next();
                    if let Some((j, y)) = gj {
                        if at == At::Start && is_start_of_word(word_def, x, y) {
                            wp = j;
                            break 'inner;
                        } else if at != At::Start && is_end_of_word(word_def, x, y) {
                            if word_def == Word::Emacs || at == At::AfterEnd {
                                wp = j;
                            } else {
                                wp = i;
                            }
                            break 'inner;
                        }
                        gi = gj;
                    } else {
                        break 'outer;
                    }
                } else {
                    break 'outer;
                }
            }
        }
        if wp == 0 {
            if word_def == Word::Emacs || at == At::AfterEnd {
                Some(self.buf.len())
            } else {
                match gi {
                    Some((i, _)) if i != 0 => Some(i + pos),
                    _ => None,
                }
            }
        } else {
            Some(wp + pos)
        }
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

    fn search_char_pos(&self, cs: CharSearch, n: RepeatCount) -> Option<usize> {
        let mut shift = 0;
        let search_result = match cs {
            CharSearch::Backward(c) | CharSearch::BackwardAfter(c) => self.buf[..self.pos]
                .char_indices()
                .rev()
                .filter(|&(_, ch)| ch == c)
                .take(n)
                .last()
                .map(|(i, _)| i),
            CharSearch::Forward(c) | CharSearch::ForwardBefore(c) => {
                if let Some(cc) = self.grapheme_at_cursor() {
                    shift = self.pos + cc.len();
                    if shift < self.buf.len() {
                        self.buf[shift..]
                            .char_indices()
                            .filter(|&(_, ch)| ch == c)
                            .take(n)
                            .last()
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
            Some(match cs {
                CharSearch::Backward(_) => pos,
                CharSearch::BackwardAfter(c) => pos + c.len_utf8(),
                CharSearch::Forward(_) => shift + pos,
                CharSearch::ForwardBefore(_) => {
                    shift + pos - self.buf[..shift + pos]
                        .chars()
                        .next_back()
                        .unwrap()
                        .len_utf8()
                }
            })
        } else {
            None
        }
    }

    /// Move cursor to the matching character position.
    /// Return `true` when the search succeeds.
    pub fn move_to(&mut self, cs: CharSearch, n: RepeatCount) -> bool {
        if let Some(pos) = self.search_char_pos(cs, n) {
            self.pos = pos;
            true
        } else {
            false
        }
    }

    /// Kill from the cursor to the end of the current word,
    /// or, if between words, to the end of the next word.
    pub fn delete_word(&mut self, at: At, word_def: Word, n: RepeatCount) -> bool {
        if let Some(pos) = self.next_word_pos(self.pos, at, word_def, n) {
            let start = self.pos;
            self.drain(start..pos, Direction::Forward);
            true
        } else {
            false
        }
    }

    pub fn delete_to(&mut self, cs: CharSearch, n: RepeatCount) -> bool {
        let search_result = match cs {
            CharSearch::ForwardBefore(c) => self.search_char_pos(CharSearch::Forward(c), n),
            _ => self.search_char_pos(cs, n),
        };
        if let Some(pos) = search_result {
            match cs {
                CharSearch::Backward(_) | CharSearch::BackwardAfter(_) => {
                    let end = self.pos;
                    self.pos = pos;
                    self.drain(pos..end, Direction::Backward);
                }
                CharSearch::ForwardBefore(_) => {
                    let start = self.pos;
                    self.drain(start..pos, Direction::Forward);
                }
                CharSearch::Forward(c) => {
                    let start = self.pos;
                    self.drain(start..pos + c.len_utf8(), Direction::Forward);
                }
            };
            true
        } else {
            false
        }
    }

    fn skip_whitespace(&self) -> Option<usize> {
        if self.pos == self.buf.len() {
            return None;
        }
        self.buf[self.pos..]
            .grapheme_indices(true)
            .filter(|&(_, ch)| ch.chars().all(|c| c.is_alphanumeric()))
            .map(|(i, _)| i)
            .next()
            .map(|i| i + self.pos)
    }

    /// Alter the next word.
    pub fn edit_word(&mut self, a: WordAction) -> bool {
        if let Some(start) = self.skip_whitespace() {
            if let Some(end) = self.next_word_pos(start, At::AfterEnd, Word::Emacs, 1) {
                if start == end {
                    return false;
                }
                let word = self
                    .drain(start..end, Direction::default())
                    .collect::<String>();
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
        self.move_to_next_word(At::AfterEnd, word_def, n);
        let w2_end = self.pos;
        self.move_to_prev_word(word_def, 1);
        let w2_beg = self.pos;
        self.move_to_prev_word(word_def, n);
        let w1_beg = self.pos;
        self.move_to_next_word(At::AfterEnd, word_def, 1);
        let w1_end = self.pos;
        if w1_beg == w2_beg || w2_beg < w1_end {
            return false;
        }

        let w1 = self.buf[w1_beg..w1_end].to_owned();

        let w2 = self
            .drain(w2_beg..w2_end, Direction::default())
            .collect::<String>();
        self.insert_str(w2_beg, &w1);

        self.drain(w1_beg..w1_end, Direction::default());
        self.insert_str(w1_beg, &w2);

        self.pos = w2_end;
        true
    }

    /// Replaces the content between [`start`..`end`] with `text`
    /// and positions the cursor to the end of text.
    pub fn replace(&mut self, range: Range<usize>, text: &str) {
        let start = range.start;
        for cl in &self.cl {
            cl.borrow_mut()
                .replace(start, self.buf.index(range.clone()), text);
        }
        self.buf.drain(range);
        if start == self.buf.len() {
            self.buf.push_str(text);
        } else {
            self.buf.insert_str(start, text);
        }
        self.pos = start + text.len();
    }

    /// Insert the `s`tring at the specified position.
    /// Return `true` if the text has been inserted at the end of the line.
    pub fn insert_str(&mut self, idx: usize, s: &str) -> bool {
        for cl in &self.cl {
            cl.borrow_mut().insert_str(idx, s);
        }
        if idx == self.buf.len() {
            self.buf.push_str(s);
            true
        } else {
            self.buf.insert_str(idx, s);
            false
        }
    }

    /// Remove the specified `range` in the line.
    pub fn delete_range(&mut self, range: Range<usize>) {
        self.set_pos(range.start);
        self.drain(range, Direction::default());
    }

    fn drain(&mut self, range: Range<usize>, dir: Direction) -> Drain {
        for dl in &self.dl {
            let mut lock = dl.try_lock();
            if let Ok(mut dl) = lock {
                dl.delete(range.start, &self.buf[range.start..range.end], dir);
            }
        }
        for cl in &self.cl {
            cl.borrow_mut()
                .delete(range.start, &self.buf[range.start..range.end], dir);
        }
        self.buf.drain(range)
    }

    /// Return the content between current cursor position and `mvt` position.
    /// Return `None` when the buffer is empty or when the movement fails.
    pub fn copy(&self, mvt: &Movement) -> Option<String> {
        if self.is_empty() {
            return None;
        }
        match *mvt {
            Movement::WholeLine => Some(self.buf.clone()),
            Movement::BeginningOfLine => if self.pos == 0 {
                None
            } else {
                Some(self.buf[..self.pos].to_owned())
            },
            Movement::ViFirstPrint => if self.pos == 0 {
                None
            } else if let Some(pos) = self.next_word_pos(0, At::Start, Word::Big, 1) {
                Some(self.buf[pos..self.pos].to_owned())
            } else {
                None
            },
            Movement::EndOfLine => if self.pos == self.buf.len() {
                None
            } else {
                Some(self.buf[self.pos..].to_owned())
            },
            Movement::BackwardWord(n, word_def) => {
                if let Some(pos) = self.prev_word_pos(self.pos, word_def, n) {
                    Some(self.buf[pos..self.pos].to_owned())
                } else {
                    None
                }
            }
            Movement::ForwardWord(n, at, word_def) => {
                if let Some(pos) = self.next_word_pos(self.pos, at, word_def, n) {
                    Some(self.buf[self.pos..pos].to_owned())
                } else {
                    None
                }
            }
            Movement::ViCharSearch(n, cs) => {
                let search_result = match cs {
                    CharSearch::ForwardBefore(c) => self.search_char_pos(CharSearch::Forward(c), n),
                    _ => self.search_char_pos(cs, n),
                };
                if let Some(pos) = search_result {
                    Some(match cs {
                        CharSearch::Backward(_) | CharSearch::BackwardAfter(_) => {
                            self.buf[pos..self.pos].to_owned()
                        }
                        CharSearch::ForwardBefore(_) => self.buf[self.pos..pos].to_owned(),
                        CharSearch::Forward(c) => self.buf[self.pos..pos + c.len_utf8()].to_owned(),
                    })
                } else {
                    None
                }
            }
            Movement::BackwardChar(n) => if let Some(pos) = self.prev_pos(n) {
                Some(self.buf[pos..self.pos].to_owned())
            } else {
                None
            },
            Movement::ForwardChar(n) => if let Some(pos) = self.next_pos(n) {
                Some(self.buf[self.pos..pos].to_owned())
            } else {
                None
            },
        }
    }

    pub fn kill(&mut self, mvt: &Movement) -> bool {
        let notify = match *mvt {
            Movement::ForwardChar(_) => false,
            Movement::BackwardChar(_) => false,
            _ => true,
        };
        if notify {
            if let Some(dl) = self.dl.as_ref() {
                let mut dl = dl.lock().unwrap();
                dl.start_killing()
            }
        }
        let killed = match *mvt {
            Movement::ForwardChar(n) => {
                // Delete (forward) `n` characters at point.
                self.delete(n).is_some()
            }
            Movement::BackwardChar(n) => {
                // Delete `n` characters backward.
                self.backspace(n)
            }
            Movement::EndOfLine => {
                // Kill the text from point to the end of the line.
                self.kill_line()
            }
            Movement::WholeLine => {
                self.move_home();
                self.kill_line()
            }
            Movement::BeginningOfLine => {
                // Kill backward from point to the beginning of the line.
                self.discard_line()
            }
            Movement::BackwardWord(n, word_def) => {
                // kill `n` words backward (until start of word)
                self.delete_prev_word(word_def, n)
            }
            Movement::ForwardWord(n, at, word_def) => {
                // kill `n` words forward (until start/end of word)
                self.delete_word(at, word_def, n)
            }
            Movement::ViCharSearch(n, cs) => self.delete_to(cs, n),
            Movement::ViFirstPrint => {
                false // TODO
            }
        };
        if notify {
            if let Some(dl) = self.dl.as_ref() {
                let mut dl = dl.lock().unwrap();
                dl.stop_killing()
            }
        }
        killed
    }
}

impl Deref for LineBuffer {
    type Target = str;

    fn deref(&self) -> &str {
        self.as_str()
    }
}

fn is_start_of_word(word_def: Word, previous: &str, grapheme: &str) -> bool {
    (!is_word_char(word_def, previous) && is_word_char(word_def, grapheme))
        || (word_def == Word::Vi && !is_other_char(previous) && is_other_char(grapheme))
}
fn is_end_of_word(word_def: Word, grapheme: &str, next: &str) -> bool {
    (!is_word_char(word_def, next) && is_word_char(word_def, grapheme))
        || (word_def == Word::Vi && !is_other_char(next) && is_other_char(grapheme))
}

fn is_word_char(word_def: Word, grapheme: &str) -> bool {
    match word_def {
        Word::Emacs => grapheme.chars().all(|c| c.is_alphanumeric()),
        Word::Vi => is_vi_word_char(grapheme),
        Word::Big => !grapheme.chars().any(|c| c.is_whitespace()),
    }
}
fn is_vi_word_char(grapheme: &str) -> bool {
    grapheme.chars().all(|c| c.is_alphanumeric()) || grapheme == "_"
}
fn is_other_char(grapheme: &str) -> bool {
    !(grapheme.chars().any(|c| c.is_whitespace()) || is_vi_word_char(grapheme))
}

#[cfg(test)]
mod test {
    use super::{ChangeListener, DeleteListener, Direction, LineBuffer, WordAction, MAX_LINE};
    use keymap::{At, CharSearch, Word};
    use std::cell::RefCell;
    use std::rc::Rc;

    struct Listener {
        deleted_str: Option<String>,
    }

    impl Listener {
        fn new() -> Rc<RefCell<Listener>> {
            let l = Listener { deleted_str: None };
            Rc::new(RefCell::new(l))
        }

        fn assert_deleted_str_eq(&self, expected: &str) {
            let actual = self.deleted_str.as_ref().expect("no deleted string");
            assert_eq!(expected, actual)
        }
    }

    impl DeleteListener for Listener {
        fn start_killing(&mut self) {}

        fn delete(&mut self, _: usize, string: &str, _: Direction) {
            self.deleted_str = Some(string.to_owned());
        }

        fn stop_killing(&mut self) {}
    }
    impl ChangeListener for Listener {
        fn insert_char(&mut self, _: usize, _: char) {}

        fn insert_str(&mut self, _: usize, _: &str) {}

        fn replace(&mut self, _: usize, _: &str, _: &str) {}
    }

    #[test]
    fn next_pos() {
        let s = LineBuffer::init("ö̲g̈", 0, None);
        assert_eq!(7, s.len());
        let pos = s.next_pos(1);
        assert_eq!(Some(4), pos);

        let s = LineBuffer::init("ö̲g̈", 4, None);
        let pos = s.next_pos(1);
        assert_eq!(Some(7), pos);
    }

    #[test]
    fn prev_pos() {
        let s = LineBuffer::init("ö̲g̈", 4, None);
        assert_eq!(7, s.len());
        let pos = s.prev_pos(1);
        assert_eq!(Some(0), pos);

        let s = LineBuffer::init("ö̲g̈", 7, None);
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
        let mut s = LineBuffer::init("αß", 2, None);
        s.move_forward(1);
        let ok = s.yank("γδε", 1);
        assert_eq!(Some(true), ok);
        assert_eq!("αßγδε", s.buf);
        assert_eq!(10, s.pos);
    }

    #[test]
    fn yank_before() {
        let mut s = LineBuffer::init("αε", 2, None);
        let ok = s.yank("ßγδ", 1);
        assert_eq!(Some(false), ok);
        assert_eq!("αßγδε", s.buf);
        assert_eq!(8, s.pos);
    }

    #[test]
    fn moves() {
        let mut s = LineBuffer::init("αß", 4, None);
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
        let mut s = LineBuffer::init("ag̈", 4, None);
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
        let cl = Listener::new();
        let mut s = LineBuffer::init("αß", 2, Some(cl.clone()));
        let chars = s.delete(1);
        assert_eq!("α", s.buf);
        assert_eq!(2, s.pos);
        assert_eq!(Some("ß".to_owned()), chars);

        let ok = s.backspace(1);
        assert_eq!("", s.buf);
        assert_eq!(0, s.pos);
        assert_eq!(true, ok);
        cl.borrow().assert_deleted_str_eq("α");
    }

    #[test]
    fn kill() {
        let cl = Listener::new();
        let mut s = LineBuffer::init("αßγδε", 6, Some(cl.clone()));
        let ok = s.kill_line();
        assert_eq!("αßγ", s.buf);
        assert_eq!(6, s.pos);
        assert_eq!(true, ok);
        cl.borrow().assert_deleted_str_eq("δε");

        s.pos = 4;
        let ok = s.discard_line();
        assert_eq!("γ", s.buf);
        assert_eq!(0, s.pos);
        assert_eq!(true, ok);
        cl.borrow().assert_deleted_str_eq("αß");
    }

    #[test]
    fn transpose() {
        let mut s = LineBuffer::init("aßc", 1, None);
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
        let mut s = LineBuffer::init("a ß  c", 6, None); // before 'c'
        let ok = s.move_to_prev_word(Word::Emacs, 1);
        assert_eq!("a ß  c", s.buf);
        assert_eq!(2, s.pos); // before 'ß'
        assert!(true, ok);

        assert!(s.move_end()); // after 'c'
        assert_eq!(7, s.pos);
        let ok = s.move_to_prev_word(Word::Emacs, 1);
        assert!(true, ok);
        assert_eq!(6, s.pos); // before 'c'

        let ok = s.move_to_prev_word(Word::Emacs, 2);
        assert!(true, ok);
        assert_eq!(0, s.pos);
    }

    #[test]
    fn move_to_prev_vi_word() {
        let mut s = LineBuffer::init("alpha ,beta/rho; mu", 19, None);
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
        let mut s = LineBuffer::init("alpha ,beta/rho; mu", 19, None);
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
        let mut s = LineBuffer::init("αßγδε", 2, None);
        let ok = s.move_to(CharSearch::ForwardBefore('ε'), 1);
        assert_eq!(true, ok);
        assert_eq!(6, s.pos);

        let mut s = LineBuffer::init("αßγδε", 2, None);
        let ok = s.move_to(CharSearch::Forward('ε'), 1);
        assert_eq!(true, ok);
        assert_eq!(8, s.pos);

        let mut s = LineBuffer::init("αßγδε", 2, None);
        let ok = s.move_to(CharSearch::Forward('ε'), 10);
        assert_eq!(true, ok);
        assert_eq!(8, s.pos);
    }

    #[test]
    fn move_to_backward() {
        let mut s = LineBuffer::init("αßγδε", 8, None);
        let ok = s.move_to(CharSearch::BackwardAfter('ß'), 1);
        assert_eq!(true, ok);
        assert_eq!(4, s.pos);

        let mut s = LineBuffer::init("αßγδε", 8, None);
        let ok = s.move_to(CharSearch::Backward('ß'), 1);
        assert_eq!(true, ok);
        assert_eq!(2, s.pos);
    }

    #[test]
    fn delete_prev_word() {
        let cl = Listener::new();
        let mut s = LineBuffer::init("a ß  c", 6, Some(cl.clone()));
        let ok = s.delete_prev_word(Word::Big, 1);
        assert_eq!("a c", s.buf);
        assert_eq!(2, s.pos);
        assert_eq!(true, ok);
        cl.borrow().assert_deleted_str_eq("ß  ");
    }

    #[test]
    fn move_to_next_word() {
        let mut s = LineBuffer::init("a ß  c", 1, None); // after 'a'
        let ok = s.move_to_next_word(At::AfterEnd, Word::Emacs, 1);
        assert_eq!("a ß  c", s.buf);
        assert_eq!(true, ok);
        assert_eq!(4, s.pos); // after 'ß'

        let ok = s.move_to_next_word(At::AfterEnd, Word::Emacs, 1);
        assert_eq!(true, ok);
        assert_eq!(7, s.pos); // after 'c'

        s.move_home();
        let ok = s.move_to_next_word(At::AfterEnd, Word::Emacs, 1);
        assert_eq!(true, ok);
        assert_eq!(1, s.pos); // after 'a'

        let ok = s.move_to_next_word(At::AfterEnd, Word::Emacs, 2);
        assert_eq!(true, ok);
        assert_eq!(7, s.pos); // after 'c'
    }

    #[test]
    fn move_to_end_of_word() {
        let mut s = LineBuffer::init("a ßeta  c", 1, None);
        let ok = s.move_to_next_word(At::BeforeEnd, Word::Vi, 1);
        assert_eq!("a ßeta  c", s.buf);
        assert_eq!(6, s.pos);
        assert_eq!(true, ok);
    }

    #[test]
    fn move_to_end_of_vi_word() {
        let mut s = LineBuffer::init("alpha ,beta/rho; mu", 0, None);
        let ok = s.move_to_next_word(At::BeforeEnd, Word::Vi, 1);
        assert!(ok);
        assert_eq!(4, s.pos);
        let ok = s.move_to_next_word(At::BeforeEnd, Word::Vi, 1);
        assert!(ok);
        assert_eq!(6, s.pos);
        let ok = s.move_to_next_word(At::BeforeEnd, Word::Vi, 1);
        assert!(ok);
        assert_eq!(10, s.pos);
        let ok = s.move_to_next_word(At::BeforeEnd, Word::Vi, 1);
        assert!(ok);
        assert_eq!(11, s.pos);
        let ok = s.move_to_next_word(At::BeforeEnd, Word::Vi, 1);
        assert!(ok);
        assert_eq!(14, s.pos);
        let ok = s.move_to_next_word(At::BeforeEnd, Word::Vi, 1);
        assert!(ok);
        assert_eq!(15, s.pos);
        let ok = s.move_to_next_word(At::BeforeEnd, Word::Vi, 1);
        assert!(ok);
        assert_eq!(18, s.pos);
        let ok = s.move_to_next_word(At::BeforeEnd, Word::Vi, 1);
        assert!(!ok);
    }

    #[test]
    fn move_to_end_of_big_word() {
        let mut s = LineBuffer::init("alpha ,beta/rho; mu", 0, None);
        let ok = s.move_to_next_word(At::BeforeEnd, Word::Big, 1);
        assert!(ok);
        assert_eq!(4, s.pos);
        let ok = s.move_to_next_word(At::BeforeEnd, Word::Big, 1);
        assert!(ok);
        assert_eq!(15, s.pos);
        let ok = s.move_to_next_word(At::BeforeEnd, Word::Big, 1);
        assert!(ok);
        assert_eq!(18, s.pos);
        let ok = s.move_to_next_word(At::BeforeEnd, Word::Big, 1);
        assert!(!ok);
    }

    #[test]
    fn move_to_start_of_word() {
        let mut s = LineBuffer::init("a ß  c", 2, None);
        let ok = s.move_to_next_word(At::Start, Word::Emacs, 1);
        assert_eq!("a ß  c", s.buf);
        assert_eq!(6, s.pos);
        assert_eq!(true, ok);
    }

    #[test]
    fn move_to_start_of_vi_word() {
        let mut s = LineBuffer::init("alpha ,beta/rho; mu", 0, None);
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
        assert_eq!(18, s.pos);
        let ok = s.move_to_next_word(At::Start, Word::Vi, 1);
        assert!(!ok);
    }

    #[test]
    fn move_to_start_of_big_word() {
        let mut s = LineBuffer::init("alpha ,beta/rho; mu", 0, None);
        let ok = s.move_to_next_word(At::Start, Word::Big, 1);
        assert!(ok);
        assert_eq!(6, s.pos);
        let ok = s.move_to_next_word(At::Start, Word::Big, 1);
        assert!(ok);
        assert_eq!(17, s.pos);
        let ok = s.move_to_next_word(At::Start, Word::Big, 1);
        assert!(ok);
        assert_eq!(18, s.pos);
        let ok = s.move_to_next_word(At::Start, Word::Big, 1);
        assert!(!ok);
    }

    #[test]
    fn delete_word() {
        let cl = Listener::new();
        let mut s = LineBuffer::init("a ß  c", 1, Some(cl.clone()));
        let ok = s.delete_word(At::AfterEnd, Word::Emacs, 1);
        assert_eq!("a  c", s.buf);
        assert_eq!(1, s.pos);
        assert_eq!(true, ok);
        cl.borrow().assert_deleted_str_eq(" ß");

        let mut s = LineBuffer::init("test", 0, Some(cl.clone()));
        let ok = s.delete_word(At::AfterEnd, Word::Vi, 1);
        assert_eq!("", s.buf);
        assert_eq!(0, s.pos);
        assert_eq!(true, ok);
        cl.borrow().assert_deleted_str_eq("test");
    }

    #[test]
    fn delete_til_start_of_word() {
        let cl = Listener::new();
        let mut s = LineBuffer::init("a ß  c", 2, Some(cl.clone()));
        let ok = s.delete_word(At::Start, Word::Emacs, 1);
        assert_eq!("a c", s.buf);
        assert_eq!(2, s.pos);
        assert_eq!(true, ok);
        cl.borrow().assert_deleted_str_eq("ß  ");
    }

    #[test]
    fn delete_to_forward() {
        let cl = Listener::new();
        let mut s = LineBuffer::init("αßγδε", 2, Some(cl.clone()));
        let ok = s.delete_to(CharSearch::ForwardBefore('ε'), 1);
        assert_eq!(true, ok);
        cl.borrow().assert_deleted_str_eq("ßγδ");
        assert_eq!("αε", s.buf);
        assert_eq!(2, s.pos);

        let mut s = LineBuffer::init("αßγδε", 2, Some(cl.clone()));
        let ok = s.delete_to(CharSearch::Forward('ε'), 1);
        assert_eq!(true, ok);
        cl.borrow().assert_deleted_str_eq("ßγδε");
        assert_eq!("α", s.buf);
        assert_eq!(2, s.pos);
    }

    #[test]
    fn delete_to_backward() {
        let cl = Listener::new();
        let mut s = LineBuffer::init("αßγδε", 8, Some(cl.clone()));
        let ok = s.delete_to(CharSearch::BackwardAfter('α'), 1);
        assert_eq!(true, ok);
        cl.borrow().assert_deleted_str_eq("ßγδ");
        assert_eq!("αε", s.buf);
        assert_eq!(2, s.pos);

        let mut s = LineBuffer::init("αßγδε", 8, Some(cl.clone()));
        let ok = s.delete_to(CharSearch::Backward('ß'), 1);
        assert_eq!(true, ok);
        cl.borrow().assert_deleted_str_eq("ßγδ");
        assert_eq!("αε", s.buf);
        assert_eq!(2, s.pos);
    }

    #[test]
    fn edit_word() {
        let mut s = LineBuffer::init("a ßeta  c", 1, None);
        assert!(s.edit_word(WordAction::UPPERCASE));
        assert_eq!("a SSETA  c", s.buf);
        assert_eq!(7, s.pos);

        let mut s = LineBuffer::init("a ßetA  c", 1, None);
        assert!(s.edit_word(WordAction::LOWERCASE));
        assert_eq!("a ßeta  c", s.buf);
        assert_eq!(7, s.pos);

        let mut s = LineBuffer::init("a ßETA  c", 1, None);
        assert!(s.edit_word(WordAction::CAPITALIZE));
        assert_eq!("a SSeta  c", s.buf);
        assert_eq!(7, s.pos);

        let mut s = LineBuffer::init("test", 1, None);
        assert!(s.edit_word(WordAction::CAPITALIZE));
        assert_eq!("tEst", s.buf);
        assert_eq!(4, s.pos);
    }

    #[test]
    fn transpose_words() {
        let mut s = LineBuffer::init("ßeta / δelta__", 15, None);
        assert!(s.transpose_words(1));
        assert_eq!("δelta__ / ßeta", s.buf);
        assert_eq!(16, s.pos);

        let mut s = LineBuffer::init("ßeta / δelta", 14, None);
        assert!(s.transpose_words(1));
        assert_eq!("δelta / ßeta", s.buf);
        assert_eq!(14, s.pos);

        let mut s = LineBuffer::init(" / δelta", 8, None);
        assert!(!s.transpose_words(1));

        let mut s = LineBuffer::init("ßeta / __", 9, None);
        assert!(!s.transpose_words(1));
    }
}
