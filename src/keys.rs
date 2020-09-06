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
pub fn char_to_key_press(c: char) -> KeyEvent {
    if !c.is_control() {
        return (KeyCode::Char(c), Modifiers::NONE); // no SHIFT even if `c` is uppercase
    }
    #[allow(clippy::match_same_arms)]
    match c {
        '\x00' => (KeyCode::Char(' '), Modifiers::CTRL),
        '\x01' => (KeyCode::Char('A'), Modifiers::CTRL),
        '\x02' => (KeyCode::Char('B'), Modifiers::CTRL),
        '\x03' => (KeyCode::Char('C'), Modifiers::CTRL),
        '\x04' => (KeyCode::Char('D'), Modifiers::CTRL),
        '\x05' => (KeyCode::Char('E'), Modifiers::CTRL),
        '\x06' => (KeyCode::Char('F'), Modifiers::CTRL),
        '\x07' => (KeyCode::Char('G'), Modifiers::CTRL),
        '\x08' => (KeyCode::Backspace, Modifiers::NONE), // '\b'
        '\x09' => (KeyCode::Tab, Modifiers::NONE),       // '\t'
        '\x0a' => (KeyCode::Char('J'), Modifiers::CTRL), // '\n' (10)
        '\x0b' => (KeyCode::Char('K'), Modifiers::CTRL),
        '\x0c' => (KeyCode::Char('L'), Modifiers::CTRL),
        '\x0d' => (KeyCode::Enter, Modifiers::NONE), // '\r' (13)
        '\x0e' => (KeyCode::Char('N'), Modifiers::CTRL),
        '\x0f' => (KeyCode::Char('O'), Modifiers::CTRL),
        '\x10' => (KeyCode::Char('P'), Modifiers::CTRL),
        '\x11' => (KeyCode::Char('Q'), Modifiers::CTRL),
        '\x12' => (KeyCode::Char('R'), Modifiers::CTRL),
        '\x13' => (KeyCode::Char('S'), Modifiers::CTRL),
        '\x14' => (KeyCode::Char('T'), Modifiers::CTRL),
        '\x15' => (KeyCode::Char('U'), Modifiers::CTRL),
        '\x16' => (KeyCode::Char('V'), Modifiers::CTRL),
        '\x17' => (KeyCode::Char('W'), Modifiers::CTRL),
        '\x18' => (KeyCode::Char('X'), Modifiers::CTRL),
        '\x19' => (KeyCode::Char('Y'), Modifiers::CTRL),
        '\x1a' => (KeyCode::Char('Z'), Modifiers::CTRL),
        '\x1b' => (KeyCode::Esc, Modifiers::NONE), // Ctrl-[
        '\x1c' => (KeyCode::Char('\\'), Modifiers::CTRL),
        '\x1d' => (KeyCode::Char(']'), Modifiers::CTRL),
        '\x1e' => (KeyCode::Char('^'), Modifiers::CTRL),
        '\x1f' => (KeyCode::Char('_'), Modifiers::CTRL),
        '\x7f' => (KeyCode::Backspace, Modifiers::NONE), // Rubout
        '\u{9b}' => (KeyCode::Esc, Modifiers::SHIFT),
        _ => (KeyCode::Null, Modifiers::NONE),
    }
}

#[cfg(test)]
mod tests {
    use super::{char_to_key_press, KeyCode, Modifiers};

    #[test]
    fn char_to_key() {
        assert_eq!((KeyCode::Esc, Modifiers::NONE), char_to_key_press('\x1b'));
    }
}
