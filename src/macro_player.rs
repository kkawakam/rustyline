//! Macro: replays keystroke sequences

#[derive(Debug, Default)]
pub struct MacroPlayer {
    buffer: Vec<char>,
    position: usize,
}

impl MacroPlayer {
    /// Start playing a new macro, stripping \r characters
    pub fn start(&mut self, macro_str: String) {
        self.buffer = macro_str.chars().filter(|&c| c != '\r').collect();
        self.position = 0;
    }
}

impl Iterator for MacroPlayer {
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        if self.position < self.buffer.len() {
            let ch = self.buffer[self.position];
            self.position += 1;
            Some(ch)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strips_carriage_returns() {
        let mut player = MacroPlayer::default();
        player.start("a\r\nb".to_string());

        assert_eq!(player.next(), Some('a'));
        assert_eq!(player.next(), Some('\n'));
        assert_eq!(player.next(), Some('b'));
        assert_eq!(player.next(), None);
    }
}
