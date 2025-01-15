use std::cmp::Ordering;

/// Height, width
pub type Unit = u16;
/// Character width / number of columns
pub(crate) fn cwidh(c: char) -> Unit {
    use unicode_width::UnicodeWidthChar;
    Unit::try_from(c.width().unwrap_or(0)).unwrap()
}
/// String width / number of columns
pub(crate) fn swidth(s: &str) -> Unit {
    use unicode_width::UnicodeWidthStr;
    Unit::try_from(s.width()).unwrap()
}

fn wcwidth(s: &str) -> Unit {
    let mut width = 0;
    for c in s.chars() {
        width += cwidh(c);
    }
    width
}

const ZWJ: char = '\u{200D}';
fn no_zwj(s: &str) -> Unit {
    let mut width = 0;
    for x in s.split(ZWJ) {
        width += swidth(x);
    }
    width
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Position {
    pub col: Unit, // The leftmost column is number 0.
    pub row: Unit, // The highest row is number 0.
}

impl PartialOrd for Position {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Position {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.row.cmp(&other.row) {
            Ordering::Equal => self.col.cmp(&other.col),
            o => o,
        }
    }
}

#[derive(Debug, Default)]
pub struct Layout {
    /// Prompt Unicode/visible width and height
    pub prompt_size: Position,
    pub default_prompt: bool,
    /// Cursor position (relative to the start of the prompt)
    pub cursor: Position,
    /// Number of rows used so far (from start of prompt to end of input)
    pub end: Position,
}

#[cfg(test)]
mod test {
    #[test]
    fn unicode_width() {
        assert_eq!(1, super::swidth("a"));
        assert_eq!(2, super::swidth("👩‍🚀"));
        assert_eq!(2, super::swidth("👋🏿"));
        assert_eq!(2, super::swidth("👨‍👩‍👧‍👦"));
        assert_eq!(2, super::swidth("👩🏼‍👨🏼‍👦🏼‍👦🏼"));
        assert_eq!(2, super::swidth("❤️"));
    }
    #[test]
    fn test_wcwidth() {
        assert_eq!(1, super::wcwidth("a"));
        assert_eq!(4, super::wcwidth("👩‍🚀"));
        assert_eq!(4, super::wcwidth("👋🏿"));
        assert_eq!(8, super::wcwidth("👨‍👩‍👧‍👦"));
        assert_eq!(16, super::wcwidth("👩🏼‍👨🏼‍👦🏼‍👦🏼"));
        assert_eq!(1, super::wcwidth("❤️"));
    }
    #[test]
    fn test_no_zwj() {
        assert_eq!(1, super::no_zwj("a"));
        assert_eq!(4, super::no_zwj("👩‍🚀"));
        assert_eq!(2, super::no_zwj("👋🏿"));
        assert_eq!(8, super::no_zwj("👨‍👩‍👧‍👦"));
        assert_eq!(8, super::no_zwj("👩🏼‍👨🏼‍👦🏼‍👦🏼"));
        assert_eq!(2, super::no_zwj("❤️"));
    }
}
