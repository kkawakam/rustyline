//! Key constants

/// Input key pressed and modifiers
pub type KeyEvent = (KeyCode, Modifiers);

/// ctrl-a => ctrl-A
/// shift-A => A
/// shift-Tab => BackTab
pub fn normalize(e: KeyEvent) -> KeyEvent {
    use {KeyCode as K, Modifiers as M};

    match e {
        (K::Char(c), m) if c.is_ascii_control() => char_to_key_press(c, m),
        (K::Char(c), m) if c.is_ascii_lowercase() && m.contains(M::CTRL) => {
            (K::Char(c.to_ascii_uppercase()), m)
        }
        (K::Char(c), m) if c.is_ascii_uppercase() && m.contains(M::SHIFT) => {
            (K::Char(c), m ^ M::SHIFT)
        }
        (K::Tab, m) if m.contains(M::SHIFT) => (K::BackTab, m ^ M::SHIFT),
        _ => e,
    }
}

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
    use {KeyCode as K, Modifiers as M};

    if !c.is_control() {
        if !mods.is_empty() {
            mods.remove(M::SHIFT); // TODO Validate: no SHIFT even if
                                   // `c` is uppercase
        }
        return (K::Char(c), mods);
    }
    #[allow(clippy::match_same_arms)]
    match c {
        '\x00' => (K::Char(' '), mods | M::CTRL),
        '\x01' => (K::Char('A'), mods | M::CTRL),
        '\x02' => (K::Char('B'), mods | M::CTRL),
        '\x03' => (K::Char('C'), mods | M::CTRL),
        '\x04' => (K::Char('D'), mods | M::CTRL),
        '\x05' => (K::Char('E'), mods | M::CTRL),
        '\x06' => (K::Char('F'), mods | M::CTRL),
        '\x07' => (K::Char('G'), mods | M::CTRL),
        '\x08' => (K::Backspace, mods), // '\b'
        '\x09' => {
            // '\t'
            if mods.contains(M::SHIFT) {
                mods.remove(M::SHIFT);
                (K::BackTab, mods)
            } else {
                (K::Tab, mods)
            }
        }
        '\x0a' => (K::Char('J'), mods | M::CTRL), // '\n' (10)
        '\x0b' => (K::Char('K'), mods | M::CTRL),
        '\x0c' => (K::Char('L'), mods | M::CTRL),
        '\x0d' => (K::Enter, mods), // '\r' (13)
        '\x0e' => (K::Char('N'), mods | M::CTRL),
        '\x0f' => (K::Char('O'), mods | M::CTRL),
        '\x10' => (K::Char('P'), mods | M::CTRL),
        '\x11' => (K::Char('Q'), mods | M::CTRL),
        '\x12' => (K::Char('R'), mods | M::CTRL),
        '\x13' => (K::Char('S'), mods | M::CTRL),
        '\x14' => (K::Char('T'), mods | M::CTRL),
        '\x15' => (K::Char('U'), mods | M::CTRL),
        '\x16' => (K::Char('V'), mods | M::CTRL),
        '\x17' => (K::Char('W'), mods | M::CTRL),
        '\x18' => (K::Char('X'), mods | M::CTRL),
        '\x19' => (K::Char('Y'), mods | M::CTRL),
        '\x1a' => (K::Char('Z'), mods | M::CTRL),
        '\x1b' => (K::Esc, mods), // Ctrl-[
        '\x1c' => (K::Char('\\'), mods | M::CTRL),
        '\x1d' => (K::Char(']'), mods | M::CTRL),
        '\x1e' => (K::Char('^'), mods | M::CTRL),
        '\x1f' => (K::Char('_'), mods | M::CTRL),
        '\x7f' => (K::Backspace, mods), // Rubout
        '\u{9b}' => (K::Esc, mods | M::SHIFT),
        _ => (K::Null, mods),
    }
}

#[cfg(test)]
mod tests {
    use super::{KeyCode as K, Modifiers as M};

    #[test]
    fn char_to_key_press() {
        assert_eq!((K::Esc, M::NONE), super::char_to_key_press('\x1b', M::NONE));
    }

    #[test]
    fn normalize() {
        assert_eq!(
            (K::Char('A'), M::CTRL),
            super::normalize((K::Char('\x01'), M::NONE))
        );
        assert_eq!(
            (K::Char('A'), M::CTRL),
            super::normalize((K::Char('a'), M::CTRL))
        );
        assert_eq!(
            (K::Char('A'), M::NONE),
            super::normalize((K::Char('A'), M::SHIFT))
        );
        assert_eq!((K::BackTab, M::NONE), super::normalize((K::Tab, M::SHIFT)));
    }
}
