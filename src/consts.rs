

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

impl KeyPress {
    pub fn from_u8(i: u8) -> KeyPress {
        match i {
            0   => KeyPress::NULL,
            1   => KeyPress::CTRL_A,
            2   => KeyPress::CTRL_B,
            3   => KeyPress::CTRL_C,
            4   => KeyPress::CTRL_D,
            5   => KeyPress::CTRL_E,
            6   => KeyPress::CTRL_F,
            8   => KeyPress::CTRL_H,
            9   => KeyPress::TAB,
            11  => KeyPress::CTRL_K,
            12  => KeyPress::CTRL_L,
            13  => KeyPress::ENTER,
            14  => KeyPress::CTRL_N,
            16  => KeyPress::CTRL_P,
            20  => KeyPress::CTRL_T,
            21  => KeyPress::CTRL_U,
            23  => KeyPress::CTRL_W,
            27  => KeyPress::ESC,
            127 => KeyPress::BACKSPACE,
            _   => KeyPress::NULL
        } 
    }
}
