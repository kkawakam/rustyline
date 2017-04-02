//! Undo API
use line_buffer::{ChangeListener, Direction, LineBuffer};
use std_unicode::str::UnicodeStr;
use unicode_segmentation::UnicodeSegmentation;

enum Change {
    Begin,
    End,
    Insert { idx: usize, text: String }, // QuotedInsert, SelfInsert, Yank
    Delete { idx: usize, text: String }, /* BackwardDeleteChar, BackwardKillWord, DeleteChar, KillLine, KillWholeLine, KillWord, UnixLikeDiscard, ViDeleteTo */
                                         //  Replace {idx: usize, old: String, new: String}, /* CapitalizeWord, Complete, DowncaseWord, Replace, TransposeChars, TransposeWords, UpcaseWord, YankPop */
}

impl Change {
    fn undo(&self, line: &mut LineBuffer) {
        match *self {
            Change::Begin | Change::End => {
                unreachable!();
            }
            Change::Insert { idx, ref text } => {
                line.delete_range(idx..idx + text.len());
            }
            Change::Delete { idx, ref text } => {
                line.insert_str(idx, text);
                line.set_pos(idx + text.len());
            }
            /*Change::Replace{idx, ref old, ref new} => {
                line.replace(idx..idx + new.len(), old);
            }*/
        }
    }

    #[cfg(test)]
    fn redo(&self, line: &mut LineBuffer) {
        match *self {
            Change::Begin | Change::End => {
                unreachable!();
            }
            Change::Insert { idx, ref text } => {
                line.insert_str(idx, text);
            }
            Change::Delete { idx, ref text } => {
                line.delete_range(idx..idx + text.len());
            }
            /*Change::Replace{idx, ref old, ref new} => {
                line.replace(idx..idx + old.len(), new);
            }*/
        }
    }

    fn insert_seq(&self, indx: usize) -> bool {
        if let Change::Insert { idx, ref text } = *self {
            idx + text.len() == indx
        } else {
            false
        }
    }

    fn delete_seq(&self, indx: usize, len: usize) -> bool {
        if let Change::Delete { idx, .. } = *self {
            // delete or backspace
            idx == indx || idx == indx + len
        } else {
            false
        }
    }
}

pub struct Changeset {
    undos: Vec<Change>, // undoable changes
    redos: Vec<Change>, // undone changes, redoable
    undoing: bool,
}

impl Changeset {
    pub fn new() -> Changeset {
        Changeset {
            undos: Vec::new(),
            redos: Vec::new(),
            undoing: false,
        }
    }

    pub fn begin(&mut self) {
        self.redos.clear();
        self.undos.push(Change::Begin);
    }

    pub fn end(&mut self) {
        self.redos.clear();
        if let Some(&Change::Begin) = self.undos.last() {
            // emtpy Begin..End
            self.undos.pop();
        } else {
            self.undos.push(Change::End);
        }
    }

    fn insert_char(idx: usize, c: char) -> Change {
        let mut text = String::new();
        text.push(c);
        Change::Insert {
            idx: idx,
            text: text,
        }
    }

    pub fn insert(&mut self, idx: usize, c: char) {
        self.redos.clear();
        if !c.is_alphanumeric() {
            self.undos.push(Self::insert_char(idx, c));
            return;
        }
        let last_change = self.undos.pop();
        match last_change {
            Some(last_change) => {
                // merge consecutive char insertions when char is alphanumeric
                if last_change.insert_seq(idx) {
                    let mut last_change = last_change;
                    if let Change::Insert { ref mut text, .. } = last_change {
                        text.push(c);
                    } else {
                        unreachable!();
                    }
                    self.undos.push(last_change);
                } else {
                    self.undos.push(last_change);
                    self.undos.push(Self::insert_char(idx, c));
                }
            }
            None => {
                self.undos.push(Self::insert_char(idx, c));
            }
        };
    }

    pub fn insert_str<S: Into<String>>(&mut self, idx: usize, string: S) {
        self.redos.clear();
        self.undos
            .push(Change::Insert {
                idx: idx,
                text: string.into(),
            });
    }

    pub fn delete<S: AsRef<str> + Into<String>>(&mut self, indx: usize, string: S) {
        self.redos.clear();

        if !Self::single_char(string.as_ref()) {
            self.undos
                .push(Change::Delete {
                    idx: indx,
                    text: string.into(),
                });
            return;
        }
        let last_change = self.undos.pop();
        match last_change {
            Some(last_change) => {
                // merge consecutive char deletions when char is alphanumeric
                if last_change.delete_seq(indx, string.as_ref().len()) {
                    let mut last_change = last_change;
                    if let Change::Delete { ref mut idx, ref mut text } = last_change {
                        if *idx == indx {
                            text.push_str(string.as_ref());
                        } else {
                            text.insert_str(0, string.as_ref());
                            *idx = indx;
                        }
                    } else {
                        unreachable!();
                    }
                    self.undos.push(last_change);
                } else {
                    self.undos.push(last_change);
                    self.undos
                        .push(Change::Delete {
                            idx: indx,
                            text: string.into(),
                        });
                }
            }
            None => {
                self.undos
                    .push(Change::Delete {
                        idx: indx,
                        text: string.into(),
                    });
            }
        };
    }

    fn single_char(s: &str) -> bool {
        let mut graphemes = s.graphemes(true);
        graphemes.next()
            .map_or(false, |grapheme| grapheme.is_alphanumeric()) &&
        graphemes.next().is_none()
    }

    /*pub fn replace<S: Into<String>>(&mut self, idx: usize, old: String, new: S) {
        self.redos.clear();
        self.undos.push(Change::Replace {
            idx: idx,
            old: old.into(),
            new: new.into(),
        });
    }*/

    pub fn undo(&mut self, line: &mut LineBuffer) -> bool {
        self.undoing = true;
        let mut waiting_for_begin = 0;
        let mut undone = false;
        loop {
            if let Some(change) = self.undos.pop() {
                match change {
                    Change::Begin => {
                        waiting_for_begin -= 1;
                    }
                    Change::End => {
                        waiting_for_begin += 1;
                    }
                    _ => {
                        change.undo(line);
                        self.redos.push(change);
                        undone = true;
                    }
                };
            } else {
                break;
            }
            if waiting_for_begin <= 0 {
                break;
            }
        }
        self.undoing = false;
        undone
    }

    #[cfg(test)]
    pub fn redo(&mut self, line: &mut LineBuffer) -> bool {
        self.undoing = true;
        let mut waiting_for_end = 0;
        let mut redone = false;
        loop {
            if let Some(change) = self.redos.pop() {
                match change {
                    Change::Begin => {
                        waiting_for_end += 1;
                    }
                    Change::End => {
                        waiting_for_end -= 1;
                    }
                    _ => {
                        change.redo(line);
                        self.undos.push(change);
                        redone = true;
                    }
                };
            } else {
                break;
            }
            if waiting_for_end <= 0 {
                break;
            }
        }
        self.undoing = false;
        redone
    }
}

impl ChangeListener for Changeset {
    fn insert_char(&mut self, idx: usize, c: char) {
        if self.undoing {
            return;
        }
        self.insert(idx, c);
    }
    fn insert_str(&mut self, idx: usize, string: &str) {
        if self.undoing {
            return;
        }
        self.insert_str(idx, string);
    }
    fn delete(&mut self, idx: usize, string: &str, _: Direction) {
        if self.undoing {
            return;
        }
        self.delete(idx, string);
    }
}

#[cfg(test)]
mod tests {
    use super::Changeset;
    use line_buffer::LineBuffer;

    #[test]
    fn test_insert_chars() {
        let mut cs = Changeset::new();
        cs.insert(0, 'H');
        cs.insert(1, 'i');
        assert_eq!(1, cs.undos.len());
        assert_eq!(0, cs.redos.len());
        cs.insert(0, ' ');
        assert_eq!(2, cs.undos.len());
    }

    #[test]
    fn test_insert_strings() {
        let mut cs = Changeset::new();
        cs.insert_str(0, "Hello");
        cs.insert_str(5, ", ");
        assert_eq!(2, cs.undos.len());
        assert_eq!(0, cs.redos.len());
    }

    #[test]
    fn test_undo_insert() {
        let mut buf = LineBuffer::init("", 0, None);
        buf.insert_str(0, "Hello");
        buf.insert_str(5, ", world!");
        let mut cs = Changeset::new();
        assert_eq!(buf.as_str(), "Hello, world!");

        cs.insert_str(5, ", world!");

        cs.undo(&mut buf);
        assert_eq!(0, cs.undos.len());
        assert_eq!(1, cs.redos.len());
        assert_eq!(buf.as_str(), "Hello");

        cs.redo(&mut buf);
        assert_eq!(1, cs.undos.len());
        assert_eq!(0, cs.redos.len());
        assert_eq!(buf.as_str(), "Hello, world!");
    }

    #[test]
    fn test_undo_delete() {
        let mut buf = LineBuffer::init("", 0, None);
        buf.insert_str(0, "Hello");
        let mut cs = Changeset::new();
        assert_eq!(buf.as_str(), "Hello");

        cs.delete(5, ", world!".to_owned());

        cs.undo(&mut buf);
        assert_eq!(buf.as_str(), "Hello, world!");

        cs.redo(&mut buf);
        assert_eq!(buf.as_str(), "Hello");
    }

    #[test]
    fn test_delete_chars() {
        let mut buf = LineBuffer::init("", 0, None);
        buf.insert_str(0, "Hlo");

        let mut cs = Changeset::new();
        cs.delete(1, "e".to_owned());
        cs.delete(1, "l".to_owned());
        assert_eq!(1, cs.undos.len());

        cs.undo(&mut buf);
        assert_eq!(buf.as_str(), "Hello");
    }

    #[test]
    fn test_backspace_chars() {
        let mut buf = LineBuffer::init("", 0, None);
        buf.insert_str(0, "Hlo");

        let mut cs = Changeset::new();
        cs.delete(2, "l".to_owned());
        cs.delete(1, "e".to_owned());
        assert_eq!(1, cs.undos.len());

        cs.undo(&mut buf);
        assert_eq!(buf.as_str(), "Hello");
    }

    /*#[test]
    fn test_undo_replace() {
        let mut buf = LineBuffer::init("", 0, None);
        buf.insert_str(0, "Hello, world!");
        let mut cs = Changeset::new();
        assert_eq!(buf.as_str(), "Hello, world!");

        buf.replace(1..5, "i");
        assert_eq!(buf.as_str(), "Hi, world!");
        cs.replace(1, "ello".to_owned(), "i");

        cs.undo(&mut buf);
        assert_eq!(buf.as_str(), "Hello, world!");

        cs.redo(&mut buf);
        assert_eq!(buf.as_str(), "Hi, world!");
    }*/
}
