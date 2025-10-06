use std::cmp::Ordering;

/// Tell how grapheme clusters are supported / rendered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphemeClusterMode {
    /// Support grapheme clustering
    Unicode,
    /// Doesn't support shaping
    WcWidth,
    /// Skip zero-width joiner
    NoZwj,
}

impl GraphemeClusterMode {
    /// Return default
    #[cfg(test)]
    pub fn from_env() -> Self {
        GraphemeClusterMode::default()
    }

    /// Use environment variables to guess current mode
    #[cfg(not(test))]
    pub fn from_env() -> Self {
        let gcm = match std::env::var("TERM_PROGRAM").as_deref() {
            Ok("Apple_Terminal") => GraphemeClusterMode::Unicode,
            Ok("iTerm.app") => GraphemeClusterMode::Unicode,
            Ok("WezTerm") => GraphemeClusterMode::Unicode,
            Err(std::env::VarError::NotPresent) => match std::env::var("TERM").as_deref() {
                Ok("xterm-kitty") => GraphemeClusterMode::NoZwj,
                _ => GraphemeClusterMode::WcWidth,
            },
            _ => GraphemeClusterMode::WcWidth,
        };
        log::debug!(target: "rustyline", "GraphemeClusterMode: {gcm:?}");
        gcm
    }

    /// Grapheme with / number of columns
    pub fn width(&self, s: &str) -> Unit {
        match self {
            GraphemeClusterMode::Unicode => uwidth(s),
            GraphemeClusterMode::WcWidth => wcwidth(s),
            GraphemeClusterMode::NoZwj => no_zwj(s),
        }
    }
}

#[cfg(test)]
#[expect(clippy::derivable_impls)]
impl Default for GraphemeClusterMode {
    fn default() -> Self {
        GraphemeClusterMode::Unicode
    }
}

/// Height, width
pub type Unit = u16;
/// Character width / number of columns
pub(crate) fn cwidh(c: char) -> Unit {
    use unicode_width::UnicodeWidthChar;
    Unit::try_from(c.width().unwrap_or(0)).unwrap()
}

fn uwidth(s: &str) -> Unit {
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
        width += uwidth(x);
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

#[derive(Debug)]
#[cfg_attr(test, derive(Default))]
pub struct Layout {
    pub grapheme_cluster_mode: GraphemeClusterMode,
    /// Prompt Unicode/visible width and height
    pub prompt_size: Position,
    pub default_prompt: bool,
    /// Cursor position (relative to the start of the prompt)
    pub cursor: Position,
    /// Number of rows used so far (from start of prompt to end of input)
    pub end: Position,
    /// Has some hint or message at the end of input
    pub has_info: bool,
}

impl Layout {
    pub fn new(grapheme_cluster_mode: GraphemeClusterMode) -> Self {
        Self {
            grapheme_cluster_mode,
            prompt_size: Position::default(),
            default_prompt: false,
            cursor: Position::default(),
            end: Position::default(),
            has_info: false,
        }
    }

    pub fn width(&self, s: &str) -> Unit {
        self.grapheme_cluster_mode.width(s)
    }
}

#[cfg(test)]
mod test {
    use crate::GraphemeClusterMode;

    #[test]
    fn unicode_width() {
        assert_eq!(1, super::uwidth("a"));
        assert_eq!(2, super::uwidth("👩‍🚀"));
        assert_eq!(2, super::uwidth("👋🏿"));
        assert_eq!(2, super::uwidth("👨‍👩‍👧‍👦"));
        // iTerm2, Terminal.app KO
        assert_eq!(2, super::uwidth("👩🏼‍👨🏼‍👦🏼‍👦🏼"));
        // WezTerm KO, Terminal.app (rendered width = 1)
        assert_eq!(2, super::uwidth("❤️"));
        let gcm = GraphemeClusterMode::Unicode;
        assert_eq!(2, gcm.width("👩🏼‍👨🏼‍👦🏼‍👦🏼"))
    }
    #[test]
    fn test_wcwidth() {
        assert_eq!(1, super::wcwidth("a"));
        assert_eq!(4, super::wcwidth("👩‍🚀"));
        assert_eq!(4, super::wcwidth("👋🏿"));
        assert_eq!(8, super::wcwidth("👨‍👩‍👧‍👦"));
        assert_eq!(16, super::wcwidth("👩🏼‍👨🏼‍👦🏼‍👦🏼"));
        assert_eq!(1, super::wcwidth("❤️"));
        let gcm = GraphemeClusterMode::WcWidth;
        assert_eq!(16, gcm.width("👩🏼‍👨🏼‍👦🏼‍👦🏼"))
    }
    #[test]
    fn test_no_zwj() {
        assert_eq!(1, super::no_zwj("a"));
        assert_eq!(4, super::no_zwj("👩‍🚀"));
        assert_eq!(2, super::no_zwj("👋🏿"));
        assert_eq!(8, super::no_zwj("👨‍👩‍👧‍👦"));
        assert_eq!(8, super::no_zwj("👩🏼‍👨🏼‍👦🏼‍👦🏼"));
        assert_eq!(2, super::no_zwj("️❤️"));
        let gcm = GraphemeClusterMode::NoZwj;
        assert_eq!(8, gcm.width("👩🏼‍👨🏼‍👦🏼‍👦🏼"))
    }
}
