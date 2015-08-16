

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum KeyPress {
    NULL        = 0,
    CTRL_A      = 1,
    CTRL_B      = 2,
    CTRL_C      = 3,
    CTRL_D      = 4,
    CTRL_E      = 5,
    CTRL_F      = 6,
    CTRL_H      = 8,
    TAB         = 9,
    CTRL_K      = 11,
    CTRL_L      = 12,
    ENTER       = 13,
    CTRL_N      = 14,
    CTRL_P      = 16,
    CTRL_T      = 20,
    CTRL_U      = 21,
    CTRL_W      = 23,
    ESC         = 27,
    BACKSPACE   = 127,
}

pub fn char_to_key_press(c: char) -> KeyPress {
    match c {
        '\x00'  => KeyPress::NULL,
        '\x01'  => KeyPress::CTRL_A,
        '\x02'  => KeyPress::CTRL_B,
        '\x03'  => KeyPress::CTRL_C,
        '\x04'  => KeyPress::CTRL_D,
        '\x05'  => KeyPress::CTRL_E,
        '\x06'  => KeyPress::CTRL_F,
        '\x08'  => KeyPress::CTRL_H,
        '\x09'  => KeyPress::TAB,
        '\x0b'  => KeyPress::CTRL_K,
        '\x0c'  => KeyPress::CTRL_L,
        '\x0d'  => KeyPress::ENTER,
        '\x0e'  => KeyPress::CTRL_N,
        '\x10'  => KeyPress::CTRL_P,
        '\x14'  => KeyPress::CTRL_T,
        '\x15'  => KeyPress::CTRL_U,
        '\x17'  => KeyPress::CTRL_W,
        '\x1b'  => KeyPress::ESC,
        '\x7f' => KeyPress::BACKSPACE,
        _   => KeyPress::NULL
    }
}
