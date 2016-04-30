//! Line buffer with current cursor position

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
        LineBuffer {
            buf: String::from(line),
            pos: pos,
        }
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
    /// Returns the character just before the current cursor position.
    fn char_before_cursor(&self) -> Option<char> {
        if self.pos == 0 {
            None
        } else {
            self.buf[..self.pos].chars().next_back()
        }
    }

    /// Insert the character `ch` at current cursor position
    /// and advance cursor position accordingly.
    /// Return `None` when maximum buffer size has been reached,
    /// `true` when the character has been appended to the end of the line.
    pub fn insert(&mut self, ch: char) -> Option<bool> {
        let shift = ch.len_utf8();
        if self.buf.len() + shift > self.buf.capacity() {
            return None;
        }
        let push = self.pos == self.buf.len();
        if push {
            self.buf.push(ch);
        } else {
            self.buf.insert(self.pos, ch);
        }
        self.pos += shift;
        Some(push)
    }

    /// Yank/paste `text` at current position.
    /// Return `None` when maximum buffer size has been reached,
    /// `true` when the character has been appended to the end of the line.
    pub fn yank(&mut self, text: &str) -> Option<bool> {
        let shift = text.len();
        if text.is_empty() || (self.buf.len() + shift) > self.buf.capacity() {
            return None;
        }
        let push = self.pos == self.buf.len();
        if self.pos == self.buf.len() {
            self.buf.push_str(text);
        } else {
            insert_str(&mut self.buf, self.pos, text);
        }
        self.pos += shift;
        Some(push)
    }

    /// Delete previously yanked text and yank/paste `text` at current position.
    pub fn yank_pop(&mut self, yank_size: usize, text: &str) -> Option<bool> {
        self.buf.drain((self.pos - yank_size)..self.pos);
        self.pos -= yank_size;
        self.yank(text)
    }

    /// Move cursor on the left.
    pub fn move_left(&mut self) -> bool {
        if let Some(ch) = self.char_before_cursor() {
            self.pos -= ch.len_utf8();
            true
        } else {
            false
        }
    }

    /// Move cursor on the right.
    pub fn move_right(&mut self) -> bool {
        if let Some(ch) = self.char_at_cursor() {
            self.pos += ch.len_utf8();
            true
        } else {
            false
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
    pub fn delete(&mut self) -> bool {
        if !self.buf.is_empty() && self.pos < self.buf.len() {
            self.buf.remove(self.pos);
            true
        } else {
            false
        }
    }

    /// Delete the character at the left of the cursor.
    /// Basically that is what happens with the "Backspace" keyboard key.
    pub fn backspace(&mut self) -> bool {
        if let Some(ch) = self.char_before_cursor() {
            self.pos -= ch.len_utf8();
            self.buf.remove(self.pos);
            true
        } else {
            false
        }
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
        if self.pos == 0 || self.pos == self.buf.len() {
            // TODO should work even if s.pos == s.buf.len()
            return false;
        }
        let ch = self.buf.remove(self.pos);
        let size = ch.len_utf8();
        let other_ch = self.char_before_cursor().unwrap();
        let other_size = other_ch.len_utf8();
        self.buf.insert(self.pos - other_size, ch);
        if self.pos != self.buf.len() - size {
            self.pos += size;
        } else if size >= other_size {
            self.pos += size - other_size;
        } else {
            self.pos -= other_size - size;
        }
        true
    }

    fn prev_word_pos<F>(&self, test: F) -> Option<usize>
        where F: Fn(char) -> bool
    {
        if self.pos == 0 {
            return None;
        }
        let mut pos = self.pos;
        // eat any spaces on the left
        pos -= self.buf[..pos]
                   .chars()
                   .rev()
                   .take_while(|ch| test(*ch))
                   .map(char::len_utf8)
                   .sum();
        if pos > 0 {
            // eat any non-spaces on the left
            pos -= self.buf[..pos]
                       .chars()
                       .rev()
                       .take_while(|ch| !test(*ch))
                       .map(char::len_utf8)
                       .sum();
        }
        Some(pos)
    }

    /// Moves the cursor to the beginning of previous word.
    pub fn move_to_prev_word(&mut self) -> bool {
        if let Some(pos) = self.prev_word_pos(|ch| !ch.is_alphanumeric()) {
            self.pos = pos;
            true
        } else {
            false
        }
    }

    /// Delete the previous word, maintaining the cursor at the start of the
    /// current word.
    pub fn delete_prev_word<F>(&mut self, test: F) -> Option<String>
        where F: Fn(char) -> bool
    {
        if let Some(pos) = self.prev_word_pos(test) {
            let text = self.buf.drain(pos..self.pos).collect();
            self.pos = pos;
            Some(text)
        } else {
            None
        }
    }

    /// Returns the position (start, end) of the next word.
    pub fn next_word_pos(&self) -> Option<(usize, usize)> {
        if self.pos < self.buf.len() {
            let mut pos = self.pos;
            // eat any spaces
            pos += self.buf[pos..]
                       .chars()
                       .take_while(|ch| !ch.is_alphanumeric())
                       .map(char::len_utf8)
                       .sum();
            let start = pos;
            if pos < self.buf.len() {
                // eat any non-spaces
                pos += self.buf[pos..]
                           .chars()
                           .take_while(|ch| ch.is_alphanumeric())
                           .map(char::len_utf8)
                           .sum();
            }
            Some((start, pos))
        } else {
            None
        }
    }

    /// Moves the cursor to the end of next word.
    pub fn move_to_next_word(&mut self) -> bool {
        if let Some((_, end)) = self.next_word_pos() {
            self.pos = end;
            true
        } else {
            false
        }
    }

    /// Kill from the cursor to the end of the current word, or, if between words, to the end of the next word.
    pub fn delete_word(&mut self) -> Option<String> {
        if let Some((_, end)) = self.next_word_pos() {
            let text = self.buf.drain(self.pos..end).collect();
            Some(text)
        } else {
            None
        }
    }

    /// Alter the next word.
    pub fn edit_word(&mut self, snapshot: &mut LineBuffer, a: WordAction) -> bool {
        if let Some((start, end)) = self.next_word_pos() {
            if start == end {
                return false;
            }
            snapshot.backup(self);
            self.buf.truncate(start);
            match a {
                WordAction::CAPITALIZE => {
                    if let Some(ch) = snapshot.buf[start..end].chars().next() {
                        let cap = ch.to_uppercase().collect::<String>();
                        self.buf.push_str(&cap);
                        self.buf.push_str(&snapshot.buf[start + ch.len_utf8()..end]);
                    } else {
                        self.buf.push_str(&snapshot.buf[start..end]);
                    }
                }
                WordAction::LOWERCASE => {
                    self.buf.push_str(&snapshot.buf[start..end].to_lowercase())
                }
                WordAction::UPPERCASE => {
                    self.buf.push_str(&snapshot.buf[start..end].to_uppercase())
                }
            };
            self.buf.push_str(&snapshot.buf[end..]);
            self.pos = end;
            true
        } else {
            false
        }
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

#[cfg(test)]
mod test {
    use super::{LineBuffer, WordAction, MAX_LINE};

    #[test]
    fn insert() {
        let mut s = LineBuffer::with_capacity(MAX_LINE);
        let push = s.insert('α').unwrap();
        assert_eq!("α", s.buf);
        assert_eq!(2, s.pos);
        assert_eq!(true, push);

        let push = s.insert('ß').unwrap();
        assert_eq!("αß", s.buf);
        assert_eq!(4, s.pos);
        assert_eq!(true, push);

        s.pos = 0;
        let push = s.insert('γ').unwrap();
        assert_eq!("γαß", s.buf);
        assert_eq!(2, s.pos);
        assert_eq!(false, push);
    }

    #[test]
    fn moves() {
        let mut s = LineBuffer::init("αß", 4);
        let ok = s.move_left();
        assert_eq!("αß", s.buf);
        assert_eq!(2, s.pos);
        assert_eq!(true, ok);

        let ok = s.move_right();
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
    fn delete() {
        let mut s = LineBuffer::init("αß", 2);
        let ok = s.delete();
        assert_eq!("α", s.buf);
        assert_eq!(2, s.pos);
        assert_eq!(true, ok);

        let ok = s.backspace();
        assert_eq!("", s.buf);
        assert_eq!(0, s.pos);
        assert_eq!(true, ok);
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
        assert_eq!(2, s.pos);
        assert_eq!(true, ok);
    }

    #[test]
    fn move_to_prev_word() {
        let mut s = LineBuffer::init("a ß  c", 6);
        let ok = s.move_to_prev_word();
        assert_eq!("a ß  c", s.buf);
        assert_eq!(2, s.pos);
        assert_eq!(true, ok);
    }

    #[test]
    fn delete_prev_word() {
        let mut s = LineBuffer::init("a ß  c", 6);
        let text = s.delete_prev_word(char::is_whitespace);
        assert_eq!("a c", s.buf);
        assert_eq!(2, s.pos);
        assert_eq!(Some("ß  ".to_string()), text);
    }

    #[test]
    fn move_to_next_word() {
        let mut s = LineBuffer::init("a ß  c", 1);
        let ok = s.move_to_next_word();
        assert_eq!("a ß  c", s.buf);
        assert_eq!(4, s.pos);
        assert_eq!(true, ok);
    }

    #[test]
    fn delete_word() {
        let mut s = LineBuffer::init("a ß  c", 1);
        let text = s.delete_word();
        assert_eq!("a  c", s.buf);
        assert_eq!(1, s.pos);
        assert_eq!(Some(" ß".to_string()), text);
    }

    #[test]
    fn edit_word() {
        let mut snapshot = LineBuffer::with_capacity(100);
        let mut s = LineBuffer::init("a ßeta  c", 1);
        assert!(s.edit_word(&mut snapshot, WordAction::UPPERCASE));
        assert_eq!("a SSETA  c", s.buf);
        assert_eq!(7, s.pos);

        let mut s = LineBuffer::init("a ßetA  c", 1);
        assert!(s.edit_word(&mut snapshot, WordAction::LOWERCASE));
        assert_eq!("a ßeta  c", s.buf);
        assert_eq!(7, s.pos);

        let mut s = LineBuffer::init("a ßeta  c", 1);
        assert!(s.edit_word(&mut snapshot, WordAction::CAPITALIZE));
        assert_eq!("a SSeta  c", s.buf);
        assert_eq!(7, s.pos);
    }
}
