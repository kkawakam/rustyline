#[derive(Debug, Clone, PartialEq, Copy)]
pub enum Key {
    Backspace,
    Char(char),
    Delete,
    Down,
    End,
    Enter, // Ctrl('M')
    Esc,
    Home,
    Insert,
    Left,
    Null,
    PageDown,
    PageUp,
    Right,
    Tab, // Ctrl('I')
    Unknown,
    Up,
}

#[derive(Debug, Clone, PartialEq, Copy)]
pub struct KeyPress {
    pub key: Key,
    pub alt: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub sup: bool,
}

macro_rules! key {
    ($key:tt) => (key!(Key::Char($key)));
    ($($key:tt)*) => (KeyPress { key: $($key)*, alt: false, ctrl: false, shift: false, sup: false });
}

macro_rules! alt {
    ($key:tt) => (alt!(Key::Char($key)));
    ($($key:tt)*) => (KeyPress { key: $($key)*, alt: true, ctrl: false, shift: false, sup: false });
}

macro_rules! ctrl {
    ($key:tt) => (ctrl!(Key::Char($key)));
    ($($key:tt)*) => (KeyPress { key: $($key)*, alt: false, ctrl: true, shift: false, sup: false });
}

macro_rules! shift {
    ($key:tt) => (shift!(Key::Char($key)));
    ($($key:tt)*) => (KeyPress { key: $($key)*, alt: false, ctrl: false, shift: true, sup: false });
}

macro_rules! sup {
    ($key:tt) => (sup!(Key::Char($key)));
    ($($key:tt)*) => (KeyPress { key: $($key)*, alt: false, ctrl: false, shift: false, sup: true });
}

#[allow(match_same_arms)]
pub fn char_to_key_press(c: char) -> KeyPress {
    if !c.is_control() {
        return key!(c);
    }

    match c {
        '\x00' => key!(Key::Null),
        '\x01' => ctrl!('A'),
        '\x02' => ctrl!('B'),
        '\x03' => ctrl!('C'),
        '\x04' => ctrl!('D'),
        '\x05' => ctrl!('E'),
        '\x06' => ctrl!('F'),
        '\x07' => ctrl!('G'),
        '\x08' => key!(Key::Backspace), // '\b'
        '\x09' => key!(Key::Tab),
        '\x0a' => ctrl!('J'), // '\n' (10)
        '\x0b' => ctrl!('K'),
        '\x0c' => ctrl!('L'),
        '\x0d' => key!(Key::Enter), // '\r' (13)
        '\x0e' => ctrl!('N'),
        '\x10' => ctrl!('P'),
        '\x12' => ctrl!('R'),
        '\x13' => ctrl!('S'),
        '\x14' => ctrl!('T'),
        '\x15' => ctrl!('U'),
        '\x16' => ctrl!('V'),
        '\x17' => ctrl!('W'),
        '\x19' => ctrl!('Y'),
        '\x1a' => ctrl!('Z'),
        '\x1b' => key!(Key::Esc),
        '\x7f' => key!(Key::Backspace), // TODO Validate
        _ => key!(Key::Null),
    }
}

#[cfg(test)]
mod tests {
    use super::{char_to_key_press, Key, KeyPress};

    #[test]
    fn char_to_key() {
        assert_eq!(key!(Key::Esc), char_to_key_press('\x1b'));
    }
}
