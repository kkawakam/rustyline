//! Undo API
use line_buffer::LineBuffer;
use unicode_segmentation::UnicodeSegmentation;

enum Action {
    Insert(String), // QuotedInsert, SelfInsert, Yank
    Delete(String), /* BackwardDeleteChar, BackwardKillWord, DeleteChar, KillLine, KillWholeLine, KillWord, UnixLikeDiscard, ViDeleteTo */
    Replace(String, String), /* CapitalizeWord, Complete, DowncaseWord, Replace, TransposeChars, TransposeWords, UpcaseWord, YankPop */
}

struct Change {
    idx: usize, // where the change happens
    action: Action,
}

impl Change {
    fn undo(&self, line: &mut LineBuffer) {
        match self.action {
            Action::Insert(ref text) => {
                line.delete_range(self.idx..self.idx + text.len());
            }
            Action::Delete(ref text) => {
                line.insert_str(self.idx, text);
                line.set_pos(self.idx + text.len());
            }
            Action::Replace(ref old, ref new) => {
                line.replace(self.idx..self.idx + new.len(), old);
            }
        }
    }

    #[cfg(test)]
    fn redo(&self, line: &mut LineBuffer) {
        match self.action {
            Action::Insert(ref text) => {
                line.insert_str(self.idx, text);
            }
            Action::Delete(ref text) => {
                line.delete_range(self.idx..self.idx + text.len());
            }
            Action::Replace(ref old, ref new) => {
                line.replace(self.idx..self.idx + old.len(), new);
            }
        }
    }

    fn insert_seq(&self, idx: usize) -> bool {
        if let Action::Insert(ref text) = self.action {
            self.idx + text.len() == idx
        } else {
            false
        }
    }

    fn delete_seq(&self, idx: usize, len: usize) -> bool {
        if let Action::Delete(_) = self.action {
            // delete or backspace
            self.idx == idx || self.idx == idx + len
        } else {
            false
        }
    }
}

pub struct Changeset {
    undos: Vec<Change>, // undoable changes
    redos: Vec<Change>, // undone changes, redoable
}

impl Changeset {
    pub fn new() -> Changeset {
        Changeset {
            undos: Vec::new(),
            redos: Vec::new(),
        }
    }

    fn insert_char(idx: usize, c: char) -> Change {
        let mut text = String::new();
        text.push(c);
        Change {
            idx: idx,
            action: Action::Insert(text),
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
                    if let Action::Insert(ref mut text) = last_change.action {
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
        self.undos.push(Change {
            idx: idx,
            action: Action::Insert(string.into()),
        });
    }

    pub fn delete<S: AsRef<str> + Into<String>>(&mut self, idx: usize, string: S) {
        self.redos.clear();

        if !Self::single_char(string.as_ref()) {
            self.undos.push(Change {
                idx: idx,
                action: Action::Delete(string.into()),
            });
            return;
        }
        let last_change = self.undos.pop();
        match last_change {
            Some(last_change) => {
                // merge consecutive char deletions when char is alphanumeric
                if last_change.delete_seq(idx, string.as_ref().len()) {
                    let mut last_change = last_change;
                    if let Action::Delete(ref mut text) = last_change.action {
                        if last_change.idx == idx {
                            text.push_str(string.as_ref());
                        } else {
                            text.insert_str(0, string.as_ref());
                            last_change.idx = idx;
                        }
                    } else {
                        unreachable!();
                    }
                    self.undos.push(last_change);
                } else {
                    self.undos.push(last_change);
                    self.undos.push(Change {
                        idx: idx,
                        action: Action::Delete(string.into()),
                    });
                }
            }
            None => {
                self.undos.push(Change {
                    idx: idx,
                    action: Action::Delete(string.into()),
                });
            }
        };
    }

    fn single_char(s: &str) -> bool {
        let mut graphemes = s.graphemes(true);
        graphemes.next().map_or(false, |grapheme| grapheme.chars().all(|c| c.is_alphanumeric())) &&
        graphemes.next().is_none()
    }

    pub fn replace<S: Into<String>>(&mut self, idx: usize, old: String, new: S) {
        self.redos.clear();
        self.undos.push(Change {
            idx: idx,
            action: Action::Replace(old.into(), new.into()),
        });
    }

    pub fn undo(&mut self, line: &mut LineBuffer) -> bool {
        match self.undos.pop() {
            Some(change) => {
                change.undo(line);
                self.redos.push(change);
                true
            }
            None => false,
        }
    }

    #[cfg(test)]
    pub fn redo(&mut self, line: &mut LineBuffer) -> bool {
        match self.redos.pop() {
            Some(change) => {
                change.redo(line);
                self.undos.push(change);
                true
            }
            None => false,
        }
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
        let mut buf = LineBuffer::init("", 0);
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
        let mut buf = LineBuffer::init("", 0);
        buf.insert_str(0, "Hello");
        let mut cs = Changeset::new();
        assert_eq!(buf.as_str(), "Hello");

        cs.delete(5, ", world!".to_string());

        cs.undo(&mut buf);
        assert_eq!(buf.as_str(), "Hello, world!");

        cs.redo(&mut buf);
        assert_eq!(buf.as_str(), "Hello");
    }

    #[test]
    fn test_delete_chars() {
        let mut buf = LineBuffer::init("", 0);
        buf.insert_str(0, "Hlo");

        let mut cs = Changeset::new();
        cs.delete(1, "e".to_string());
        cs.delete(1, "l".to_string());
        assert_eq!(1, cs.undos.len());

        cs.undo(&mut buf);
        assert_eq!(buf.as_str(), "Hello");
    }

    #[test]
    fn test_backspace_chars() {
        let mut buf = LineBuffer::init("", 0);
        buf.insert_str(0, "Hlo");

        let mut cs = Changeset::new();
        cs.delete(2, "l".to_string());
        cs.delete(1, "e".to_string());
        assert_eq!(1, cs.undos.len());

        cs.undo(&mut buf);
        assert_eq!(buf.as_str(), "Hello");
    }

    #[test]
    fn test_undo_replace() {
        let mut buf = LineBuffer::init("", 0);
        buf.insert_str(0, "Hello, world!");
        let mut cs = Changeset::new();
        assert_eq!(buf.as_str(), "Hello, world!");

        buf.replace(1..5, "i");
        assert_eq!(buf.as_str(), "Hi, world!");
        cs.replace(1, "ello".to_string(), "i");

        cs.undo(&mut buf);
        assert_eq!(buf.as_str(), "Hello, world!");

        cs.redo(&mut buf);
        assert_eq!(buf.as_str(), "Hi, world!");
    }
}
