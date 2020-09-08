//! Key constants

/// Input key pressed and modifiers
pub type KeyEvent = (KeyCode, Modifiers);

/// Input key pressed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum KeyCode {
    /// Unsupported escape sequence (on unix platform)
    UnknownEscSeq,
    /// ⌫ or Ctrl-H
    Backspace,
    /// ⇤ (usually Shift-Tab)
    BackTab,
    /// Paste (on unix platform)
    BracketedPasteStart,
    /// Paste (on unix platform)
    BracketedPasteEnd,
    /// Single char
    Char(char),
    /// ⌦
    Delete,
    /// ↓ arrow key
    Down,
    /// ⇲
    End,
    /// ↵ or Ctrl-M
    Enter,
    /// Escape or Ctrl-[
    Esc,
    /// Function key
    F(u8),
    /// ⇱
    Home,
    /// Insert key
    Insert,
    /// ← arrow key
    Left,
    /// \0
    Null,
    /// ⇟
    PageDown,
    /// ⇞
    PageUp,
    /// → arrow key
    Right,
    /// ⇥ or Ctrl-I
    Tab,
    /// ↑ arrow key
    Up,
}

bitflags::bitflags! {
    /// The set of modifier keys that were triggered along with a key press.
    pub struct Modifiers: u8 {
        /// Control modifier
        const CTRL  = 1<<3;
        /// Escape or Alt modifier
        const ALT  = 1<<2;
        /// Shift modifier
        const SHIFT = 1<<1;

        /// No modifier
        const NONE = 0;
        /// Ctrl + Shift
        const CTRL_SHIFT = Self::CTRL.bits | Self::SHIFT.bits;
        /// Alt + Shift
        const ALT_SHIFT = Self::ALT.bits | Self::SHIFT.bits;
        /// Ctrl + Alt
        const CTRL_ALT = Self::CTRL.bits | Self::ALT.bits;
        /// Ctrl + Alt + Shift
        const CTRL_ALT_SHIFT = Self::CTRL.bits | Self::ALT.bits | Self::SHIFT.bits;
    }
}

#[cfg(any(windows, unix))]
pub fn char_to_key_press(c: char, mut mods: Modifiers) -> KeyEvent {
    if !c.is_control() {
        if !mods.is_empty() {
            mods.remove(Modifiers::SHIFT); // TODO Validate: no SHIFT even if
                                           // `c` is uppercase
        }
        return (KeyCode::Char(c), mods);
    }
    #[allow(clippy::match_same_arms)]
    match c {
        '\x00' => (KeyCode::Char(' '), mods | Modifiers::CTRL),
        '\x01' => (KeyCode::Char('A'), mods | Modifiers::CTRL),
        '\x02' => (KeyCode::Char('B'), mods | Modifiers::CTRL),
        '\x03' => (KeyCode::Char('C'), mods | Modifiers::CTRL),
        '\x04' => (KeyCode::Char('D'), mods | Modifiers::CTRL),
        '\x05' => (KeyCode::Char('E'), mods | Modifiers::CTRL),
        '\x06' => (KeyCode::Char('F'), mods | Modifiers::CTRL),
        '\x07' => (KeyCode::Char('G'), mods | Modifiers::CTRL),
        '\x08' => (KeyCode::Backspace, mods), // '\b'
        '\x09' => {
            // '\t'
            if mods.contains(Modifiers::SHIFT) {
                mods.remove(Modifiers::SHIFT);
                (KeyCode::BackTab, mods)
            } else {
                (KeyCode::Tab, mods)
            }
        }
        '\x0a' => (KeyCode::Char('J'), mods | Modifiers::CTRL), // '\n' (10)
        '\x0b' => (KeyCode::Char('K'), mods | Modifiers::CTRL),
        '\x0c' => (KeyCode::Char('L'), mods | Modifiers::CTRL),
        '\x0d' => (KeyCode::Enter, mods), // '\r' (13)
        '\x0e' => (KeyCode::Char('N'), mods | Modifiers::CTRL),
        '\x0f' => (KeyCode::Char('O'), mods | Modifiers::CTRL),
        '\x10' => (KeyCode::Char('P'), mods | Modifiers::CTRL),
        '\x11' => (KeyCode::Char('Q'), mods | Modifiers::CTRL),
        '\x12' => (KeyCode::Char('R'), mods | Modifiers::CTRL),
        '\x13' => (KeyCode::Char('S'), mods | Modifiers::CTRL),
        '\x14' => (KeyCode::Char('T'), mods | Modifiers::CTRL),
        '\x15' => (KeyCode::Char('U'), mods | Modifiers::CTRL),
        '\x16' => (KeyCode::Char('V'), mods | Modifiers::CTRL),
        '\x17' => (KeyCode::Char('W'), mods | Modifiers::CTRL),
        '\x18' => (KeyCode::Char('X'), mods | Modifiers::CTRL),
        '\x19' => (KeyCode::Char('Y'), mods | Modifiers::CTRL),
        '\x1a' => (KeyCode::Char('Z'), mods | Modifiers::CTRL),
        '\x1b' => (KeyCode::Esc, mods), // Ctrl-[
        '\x1c' => (KeyCode::Char('\\'), mods | Modifiers::CTRL),
        '\x1d' => (KeyCode::Char(']'), mods | Modifiers::CTRL),
        '\x1e' => (KeyCode::Char('^'), mods | Modifiers::CTRL),
        '\x1f' => (KeyCode::Char('_'), mods | Modifiers::CTRL),
        '\x7f' => (KeyCode::Backspace, mods), // Rubout
        '\u{9b}' => (KeyCode::Esc, mods | Modifiers::SHIFT),
        _ => (KeyCode::Null, mods),
    }
}

#[cfg(test)]
mod tests {
    use super::{char_to_key_press, KeyCode, Modifiers};

    #[test]
    fn char_to_key() {
        assert_eq!(
            (KeyCode::Esc, Modifiers::NONE),
            char_to_key_press('\x1b', Modifiers::NONE)
        );
    }
}
