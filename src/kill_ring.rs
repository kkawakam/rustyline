//! Kill Ring

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Kill,
    Yank(usize),
    Other,
}

pub struct KillRing {
    slots: Vec<String>,
    index: usize,
    last_action: Action,
}

impl KillRing {
    /// Create a new kill-ring of the given `size`.
    pub fn new(size: usize) -> KillRing {
        KillRing {
            slots: Vec::with_capacity(size),
            index: 0,
            last_action: Action::Other,
        }
    }

    /// Reset last_astion state.
    pub fn reset(&mut self) {
        self.last_action = Action::Other;
    }

    /// Add `text` to the kill-ring.
    pub fn kill(&mut self, text: &str, forward: bool) {
        match self.last_action {
            Action::Kill => {
                if self.slots.capacity() == 0 {
                    // disabled
                    return;
                }
                if forward {
                    // append
                    self.slots[self.index].push_str(text);
                } else {
                    // prepend
                    self.slots[self.index] = String::from(text) + &self.slots[self.index];
                }
            }
            _ => {
                self.last_action = Action::Kill;
                if self.slots.capacity() == 0 {
                    // disabled
                    return;
                }
                if self.index == self.slots.capacity() - 1 {
                    // full
                    self.index = 0;
                } else if !self.slots.is_empty() {
                    self.index += 1;
                }
                if self.index == self.slots.len() {
                    self.slots.push(String::from(text))
                } else {
                    self.slots[self.index] = String::from(text);
                }
            }
        }
    }

    /// Yank previously killed text.
    /// Return `None` when kill-ring is empty.
    pub fn yank(&mut self) -> Option<&String> {
        if self.slots.len() == 0 {
            None
        } else {
            self.last_action = Action::Yank(self.slots[self.index].len());
            Some(&self.slots[self.index])
        }
    }

    /// Yank killed text stored in previous slot.
    /// Return `None` when the previous command was not a yank.
    pub fn yank_pop(&mut self) -> Option<(usize, &String)> {
        match self.last_action {
            Action::Yank(yank_size) => {
                if self.slots.len() == 0 {
                    return None;
                }
                if self.index == 0 {
                    self.index = self.slots.len() - 1;
                } else {
                    self.index -= 1;
                }
                self.last_action = Action::Yank(self.slots[self.index].len());
                Some((yank_size, &self.slots[self.index]))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Action, KillRing};

    #[test]
    fn disabled() {
        let mut kill_ring = KillRing::new(0);
        kill_ring.kill("text", true);
        assert!(kill_ring.slots.is_empty());
        assert_eq!(0, kill_ring.index);
        assert_eq!(Action::Kill, kill_ring.last_action);

        assert_eq!(None, kill_ring.yank());
        assert_eq!(Action::Kill, kill_ring.last_action);
    }

    #[test]
    fn one_kill() {
        let mut kill_ring = KillRing::new(2);
        kill_ring.kill("word1", true);
        assert_eq!(0, kill_ring.index);
        assert_eq!(1, kill_ring.slots.len());
        assert_eq!("word1", kill_ring.slots[0]);
        assert_eq!(Action::Kill, kill_ring.last_action);
    }

    #[test]
    fn kill_kill_forward() {
        let mut kill_ring = KillRing::new(2);
        kill_ring.kill("word1", true);
        kill_ring.kill(" word2", true);
        assert_eq!(0, kill_ring.index);
        assert_eq!(1, kill_ring.slots.len());
        assert_eq!("word1 word2", kill_ring.slots[0]);
        assert_eq!(Action::Kill, kill_ring.last_action);
    }

    #[test]
    fn kill_kill_backward() {
        let mut kill_ring = KillRing::new(2);
        kill_ring.kill("word1", false);
        kill_ring.kill("word2 ", false);
        assert_eq!(0, kill_ring.index);
        assert_eq!(1, kill_ring.slots.len());
        assert_eq!("word2 word1", kill_ring.slots[0]);
        assert_eq!(Action::Kill, kill_ring.last_action);
    }

    #[test]
    fn kill_other_kill() {
        let mut kill_ring = KillRing::new(2);
        kill_ring.kill("word1", true);
        kill_ring.reset();
        kill_ring.kill("word2", true);
        assert_eq!(1, kill_ring.index);
        assert_eq!(2, kill_ring.slots.len());
        assert_eq!("word1", kill_ring.slots[0]);
        assert_eq!("word2", kill_ring.slots[1]);
        assert_eq!(Action::Kill, kill_ring.last_action);
    }

    #[test]
    fn many_kill() {
        let mut kill_ring = KillRing::new(2);
        kill_ring.kill("word1", true);
        kill_ring.reset();
        kill_ring.kill("word2", true);
        kill_ring.reset();
        kill_ring.kill("word3", true);
        kill_ring.reset();
        kill_ring.kill("word4", true);
        assert_eq!(1, kill_ring.index);
        assert_eq!(2, kill_ring.slots.len());
        assert_eq!("word3", kill_ring.slots[0]);
        assert_eq!("word4", kill_ring.slots[1]);
        assert_eq!(Action::Kill, kill_ring.last_action);
    }

    #[test]
    fn yank() {
        let mut kill_ring = KillRing::new(2);
        kill_ring.kill("word1", true);
        kill_ring.reset();
        kill_ring.kill("word2", true);

        assert_eq!(Some(&"word2".to_string()), kill_ring.yank());
        assert_eq!(Action::Yank(5), kill_ring.last_action);
        assert_eq!(Some(&"word2".to_string()), kill_ring.yank());
        assert_eq!(Action::Yank(5), kill_ring.last_action);
    }

    #[test]
    fn yank_pop() {
        let mut kill_ring = KillRing::new(2);
        kill_ring.kill("word1", true);
        kill_ring.reset();
        kill_ring.kill("longword2", true);

        assert_eq!(None, kill_ring.yank_pop());
        kill_ring.yank();
        assert_eq!(Some((9, &"word1".to_string())), kill_ring.yank_pop());
        assert_eq!(Some((5, &"longword2".to_string())), kill_ring.yank_pop());
        assert_eq!(Some((9, &"word1".to_string())), kill_ring.yank_pop());
    }
}

// Ctrl-K -> delete to kill ring (forward) (killLine)
// Ctrl-U -> erase to kill ring (backward) (resetLine)
// Ctrl-W -> erase word to kill ring (backward) (unixWordRubout)
//
// Meta Ctrl-H -> deletePreviousWord
// Meta Delete -> deletePreviousWord
// Meta D -> deleteNextWord
// Meta-Y/y -> yankPop
//
// Ctrl-Y -> paste from buffer (yank)
//
// resetLine
// unixWordRubout
// deletePreviousWord
// deleteNextWord
// killLine
//
// yank
// yankPop
//
// echo hello world
// Ctrl-W Ctrl-W Ctrl-Y
// echo hello
// echo
// echo hello world
//
// echo hello world
// Ctrl-W
// echo hello rust
// Ctrl-W Ctrl-Y Meta-Y
// echo hello world
// Meta-Y
//
