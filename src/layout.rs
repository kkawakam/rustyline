#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Position {
    pub col: usize,
    pub row: usize,
}

#[derive(Clone, Debug, Default)]
pub struct Layout {
    /// Prompt Unicode/visible width and height
    pub prompt_size: Position,
    pub default_prompt: bool,
    /// Cursor position (relative to the start of the prompt for `row`)
    pub cursor: Position,
    /// Number of rows used so far (from start of prompt to end of input)
    pub end: Position,
}

impl Layout {
    pub fn new(prompt_size: Position, default_prompt: bool) -> Self {
        Layout {
            prompt_size,
            default_prompt,
            cursor: prompt_size,
            end: prompt_size,
        }
    }
}
