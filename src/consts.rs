//! Key constants

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyPress {
    UnknownEscSeq,
    Backspace,
    Char(char),
    Ctrl(char),
    Delete,
    Down,
    End,
    Enter, // Ctrl('M')
    Esc,
    Home,
    Left,
    Meta(char),
    Null,
    PageDown,
    PageUp,
    Right,
    Tab, // Ctrl('I')
    Up,
}

#[allow(match_same_arms)]
pub fn char_to_key_press(c: char) -> KeyPress {
    if !c.is_control() {
        return KeyPress::Char(c);
    }
    match c {
        '\x00' => KeyPress::Null,
        '\x01' => KeyPress::Ctrl('A'),
        '\x02' => KeyPress::Ctrl('B'),
        '\x03' => KeyPress::Ctrl('C'),
        '\x04' => KeyPress::Ctrl('D'),
        '\x05' => KeyPress::Ctrl('E'),
        '\x06' => KeyPress::Ctrl('F'),
        '\x07' => KeyPress::Ctrl('G'),
        '\x08' => KeyPress::Backspace, // '\b'
        '\x09' => KeyPress::Tab, // '\t'
        '\x0a' => KeyPress::Ctrl('J'), // '\n' (10)
        '\x0b' => KeyPress::Ctrl('K'),
        '\x0c' => KeyPress::Ctrl('L'),
        '\x0d' => KeyPress::Enter, // '\r' (13)
        '\x0e' => KeyPress::Ctrl('N'),
        '\x10' => KeyPress::Ctrl('P'),
        '\x12' => KeyPress::Ctrl('R'),
        '\x13' => KeyPress::Ctrl('S'),
        '\x14' => KeyPress::Ctrl('T'),
        '\x15' => KeyPress::Ctrl('U'),
        '\x16' => KeyPress::Ctrl('V'),
        '\x17' => KeyPress::Ctrl('W'),
        '\x18' => KeyPress::Ctrl('X'),
        '\x19' => KeyPress::Ctrl('Y'),
        '\x1a' => KeyPress::Ctrl('Z'),
        '\x1b' => KeyPress::Esc, // Ctrl-[
        '\x1f' => KeyPress::Ctrl('_'),
        '\x7f' => KeyPress::Backspace, // Rubout
        _ => KeyPress::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::{char_to_key_press, KeyPress};

    #[test]
    fn char_to_key() {
        assert_eq!(KeyPress::Esc, char_to_key_press('\x1b'));
    }
}
