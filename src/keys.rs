//! Key constants

/// A key, independent of any modifiers. See KeyPress for the equivalent with modifers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    Char(char),
    UnknownEscSeq,
    Backspace, // Ctrl('H')
    BackTab,
    BracketedPasteStart,
    BracketedPasteEnd,
    Delete,
    Down,
    End,
    Enter, // Ctrl('M')
    Esc,   // Ctrl('[')
    F(u8),
    Home,
    Insert,
    Left,
    Null,
    PageDown,
    PageUp,
    Right,
    Tab, // Ctrl('I')
    Up,
    #[doc(hidden)]
    __NonExhaustive,
}

impl From<char> for Key {
    #[inline]
    fn from(c: char) -> Self {
        Self::Char(c)
    }
}

impl Key {
    #[inline]
    pub const fn with_mods(self, mods: KeyMods) -> KeyPress {
        KeyPress::new(self, mods)
    }

    #[inline]
    pub const fn shift(self) -> KeyPress {
        self.with_mods(KeyMods::SHIFT)
    }

    #[inline]
    pub const fn ctrl(self) -> KeyPress {
        self.with_mods(KeyMods::CTRL)
    }

    #[inline]
    pub const fn meta(self) -> KeyPress {
        self.with_mods(KeyMods::META)
    }

    #[inline]
    pub const fn meta_shift(self) -> KeyPress {
        self.with_mods(KeyMods::META_SHIFT)
    }

    #[inline]
    pub const fn ctrl_shift(self) -> KeyPress {
        self.with_mods(KeyMods::CTRL_SHIFT)
    }

    #[inline]
    pub const fn ctrl_meta_shift(self) -> KeyPress {
        self.with_mods(KeyMods::CTRL_META_SHIFT)
    }
}

bitflags::bitflags! {
    /// The set of modifier keys that were triggered along with a key press.
    pub struct KeyMods: u8 {
        const CTRL  = 0b0001;
        const META  = 0b0010;
        const SHIFT = 0b0100;
        // TODO: Should there be an `ALT`?

        const NONE = 0;
        const CTRL_SHIFT = Self::CTRL.bits | Self::META.bits;
        const META_SHIFT = Self::META.bits | Self::SHIFT.bits;
        const CTRL_META = Self::META.bits | Self::CTRL.bits;
        const CTRL_META_SHIFT = Self::META.bits | Self::CTRL.bits | Self::SHIFT.bits;
    }
}

impl KeyMods {
    #[inline]
    pub fn ctrl_meta_shift(ctrl: bool, meta: bool, shift: bool) -> Self {
        (if ctrl { Self::CTRL } else { Self::NONE })
            | (if meta { Self::META } else { Self::NONE })
            | (if shift { Self::SHIFT } else { Self::NONE })
    }
}

/// A key press is key and some modifiers. Note that there's overlap between
/// keys (ctrl-H and delete, for example), and not all keys may be represented.
///
/// See the [`key_press!`] macro which allows using these in a pattern (e.g. a
/// match / if let binding) conveniently, but can also be used to construct
/// KeyPress values.
///
/// ## Notes
/// - for a `Key::Char` with modifiers, the upper-case character should be used.
///   e.g. `key_press!(CTRL, 'A')` and not `key_press!(CTRL, 'a')`.
/// - Upper-case letters generally will not have `KeyMods::SHIFT` associated with
///   them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyPress {
    pub key: Key,
    pub mods: KeyMods,
}

impl From<Key> for KeyPress {
    #[inline]
    fn from(k: Key) -> Self {
        Self::new(k, KeyMods::NONE)
    }
}
impl From<char> for KeyPress {
    #[inline]
    fn from(k: char) -> Self {
        Self::new(k.into(), KeyMods::NONE)
    }
}

impl KeyPress {
    #[inline]
    pub const fn new(key: Key, mods: KeyMods) -> Self {
        Self { key, mods }
    }

    #[inline]
    pub fn normal(k: impl Into<Key>) -> Self {
        Self::new(k.into(), KeyMods::NONE)
    }

    #[inline]
    pub fn ctrl(k: impl Into<Key>) -> Self {
        Self::new(k.into(), KeyMods::CTRL)
    }

    #[inline]
    pub fn shift(k: impl Into<Key>) -> Self {
        Self::new(k.into(), KeyMods::SHIFT)
    }

    #[inline]
    pub fn meta(k: impl Into<Key>) -> Self {
        Self::new(k.into(), KeyMods::META)
    }

    // These are partially here because make updating a lot easier (just turn
    // `KeyPress::Home` into `KeyPress::HOME`). OTOH,

    /// Constant value representing an unmodified press of `Key::Backspace`.
    pub const BACKSPACE: Self = Self::new(Key::Backspace, KeyMods::NONE);

    /// Constant value representing an unmodified press of `Key::BackTab`.
    pub const BACK_TAB: Self = Self::new(Key::BackTab, KeyMods::NONE);

    /// Constant value representing an unmodified press of `Key::Delete`.
    pub const DELETE: Self = Self::new(Key::Delete, KeyMods::NONE);

    /// Constant value representing an unmodified press of `Key::Down`.
    pub const DOWN: Self = Self::new(Key::Down, KeyMods::NONE);

    /// Constant value representing an unmodified press of `Key::End`.
    pub const END: Self = Self::new(Key::End, KeyMods::NONE);

    /// Constant value representing an unmodified press of `Key::Enter`.
    pub const ENTER: Self = Self::new(Key::Enter, KeyMods::NONE);

    /// Constant value representing an unmodified press of `Key::Esc`.
    pub const ESC: Self = Self::new(Key::Esc, KeyMods::NONE);

    /// Constant value representing an unmodified press of `Key::Home`.
    pub const HOME: Self = Self::new(Key::Home, KeyMods::NONE);

    /// Constant value representing an unmodified press of `Key::Insert`.
    pub const INSERT: Self = Self::new(Key::Insert, KeyMods::NONE);

    /// Constant value representing an unmodified press of `Key::Left`.
    pub const LEFT: Self = Self::new(Key::Left, KeyMods::NONE);

    /// Constant value representing an unmodified press of `Key::PageDown`.
    pub const PAGE_DOWN: Self = Self::new(Key::PageDown, KeyMods::NONE);

    /// Constant value representing an unmodified press of `Key::PageUp`.
    pub const PAGE_UP: Self = Self::new(Key::PageUp, KeyMods::NONE);

    /// Constant value representing an unmodified press of `Key::Right`.
    pub const RIGHT: Self = Self::new(Key::Right, KeyMods::NONE);

    /// Constant value representing an unmodified press of `Key::Tab`.
    pub const TAB: Self = Self::new(Key::Tab, KeyMods::NONE);

    /// Constant value representing an unmodified press of `Key::Up`.
    pub const UP: Self = Self::new(Key::Up, KeyMods::NONE);
}

/// Macro to work around the fact that you can't use the result of a function as
/// a pattern. This basically exists so that you can match on the result.
///
/// # Usage:
/// ```
/// # use rustyline::{key_press, KeyPress};
/// fn handle_key_press(k: KeyPress) {
///     match k {
///         key_press!('c') => println!("C was pressed"),
///         key_press!(CTRL, 'o') => println!("CTRL-o was pressed"),
///         key_press!(Right) => println!("Right was pressed"),
///         key_press!(SHIFT, Left) => println!("Shift-left were pressed"),
///         key_press!(Char(c)) => println!("the char {} was pressed", c),
///         key_press!(META, c @ '0'..='9') | key_press!(c @ 'a'..='z') => {
///             # let _ = c;
///             println!("You get the idea...");
///         }
///         _ => {}
///     }
/// }
/// ```
#[macro_export]
macro_rules! key_press {
    // This is written pretty weirdly but I couldn't find a less verbose way of
    // doing it (keep in mind that the macro expander doesn't backtrack and
    // accepts the first thing that could match). It also makes the code many,
    // many times more readable, IMO.

    // That said... This macro itself? Not that readable.

    (Backspace) => { $crate::KeyPress { key: $crate::Key::Backspace, mods: $crate::KeyMods::NONE } };
    (BackTab) => { $crate::KeyPress { key: $crate::Key::BackTab, mods: $crate::KeyMods::NONE } };
    (Delete) => { $crate::KeyPress { key: $crate::Key::Delete, mods: $crate::KeyMods::NONE } };
    (Down) => { $crate::KeyPress { key: $crate::Key::Down, mods: $crate::KeyMods::NONE } };
    (End) => { $crate::KeyPress { key: $crate::Key::End, mods: $crate::KeyMods::NONE } };
    (Enter) => { $crate::KeyPress { key: $crate::Key::Enter, mods: $crate::KeyMods::NONE } };
    (Esc) => { $crate::KeyPress { key: $crate::Key::Esc, mods: $crate::KeyMods::NONE } };
    (Home) => { $crate::KeyPress { key: $crate::Key::Home, mods: $crate::KeyMods::NONE } };
    (Insert) => { $crate::KeyPress { key: $crate::Key::Insert, mods: $crate::KeyMods::NONE } };
    (Left) => { $crate::KeyPress { key: $crate::Key::Left, mods: $crate::KeyMods::NONE } };
    (PageDown) => { $crate::KeyPress { key: $crate::Key::PageDown, mods: $crate::KeyMods::NONE } };
    (PageUp) => { $crate::KeyPress { key: $crate::Key::PageUp, mods: $crate::KeyMods::NONE } };
    (Right) => { $crate::KeyPress { key: $crate::Key::Right, mods: $crate::KeyMods::NONE } };
    (Tab) => { $crate::KeyPress { key: $crate::Key::Tab, mods: $crate::KeyMods::NONE } };
    (Up) => { $crate::KeyPress { key: $crate::Key::Up, mods: $crate::KeyMods::NONE } };
    (Null) => { $crate::KeyPress { key: $crate::Key::Null, mods: $crate::KeyMods::NONE }};
    (UnknownEscSeq) => { $crate::KeyPress { key: $crate::Key::UnknownEscSeq, mods: $crate::KeyMods::NONE } };
    (BracketedPasteStart) => { $crate::KeyPress { key: $crate::Key::BracketedPasteStart, mods: $crate::KeyMods::NONE }};
    (BracketedPasteEnd) => { $crate::KeyPress { key: $crate::Key::BracketedPasteEnd, mods: $crate::KeyMods::NONE }};

    (Char($($c:tt)*)) => { $crate::KeyPress { key: $crate::Key::Char($($c)*), mods: $crate::KeyMods::NONE } };
    (F($($c:tt)*)) => { $crate::KeyPress { key: $crate::Key::F($($c)*), mods: $crate::KeyMods::NONE } };

    (SHIFT, $($t:tt)*) => { $crate::KeyPress { key: $crate::key_press!(@__just_key $($t)*), mods: $crate::KeyMods::SHIFT } };
    (CTRL, $($t:tt)*) => { $crate::KeyPress { key: $crate::key_press!(@__just_key $($t)*), mods: $crate::KeyMods::CTRL } };
    (META, $($t:tt)*) => { $crate::KeyPress { key: $crate::key_press!(@__just_key $($t)*), mods: $crate::KeyMods::META } };

    (NONE, $($t:tt)*) => { $crate::KeyPress { key: $crate::key_press!(@__just_key $($t)*), mods: $crate::KeyMods::NONE } };
    (CTRL_SHIFT, $($t:tt)*) => { $crate::KeyPress { key: $crate::key_press!(@__just_key $($t)*), mods: $crate::KeyMods::CTRL_SHIFT } };
    (META_SHIFT, $($t:tt)*) => { $crate::KeyPress { key: $crate::key_press!(@__just_key $($t)*), mods: $crate::KeyMods::META_SHIFT } };
    (CTRL_META, $($t:tt)*) => { $crate::KeyPress { key: $crate::key_press!(@__just_key $($t)*), mods: $crate::KeyMods::CTRL_META } };
    (CTRL_META_SHIFT, $($t:tt)*) => { $crate::KeyPress { key: $crate::key_press!(@__just_key $($t)*), mods: $crate::KeyMods::CTRL_META_SHIFT } };
    (_, $($t:tt)*) => { $crate::KeyPress { key: $crate::key_press!(@__just_key $($t)*), mods: _ } };

    // To easy to accidentally bind this if you get `SHIFT_CTRL` and `CTRL_SHIFT` mixed up...
    // ($mods:pat, $($t:tt)*) => { $crate::KeyPress { key: $crate::key_press!(@__just_key $($t)*), mods: $mods } };

    ($ch:literal) => { $crate::KeyPress { key: $crate::Key::Char($ch), mods: $crate::KeyMods::NONE } };

    // Allow patterns like `key_press(d @ '0'..='9')`, but explicitly forbid
    // idents as it's confusing (when does `if let key_press!(c) = some_key {}`
    // match?)
    ($ch:ident) => { compile_error!("`key_press!(c)` is ambiguous, try `key_press!(Char(c))`?") };

    ($ch:pat) => { $crate::KeyPress { key: $crate::Key::Char($ch), mods: $crate::KeyMods::NONE } };

    // Has to be duplicated. (Forwarding the above to this causes it to fail to
    // match...)
    (@__just_key Backspace) => { $crate::Key::Backspace };
    (@__just_key BackTab) => { $crate::Key::BackTab };
    (@__just_key Delete) => { $crate::Key::Delete };
    (@__just_key Down) => { $crate::Key::Down };
    (@__just_key End) => { $crate::Key::End };
    (@__just_key Enter) => { $crate::Key::Enter };
    (@__just_key Esc) => { $crate::Key::Esc };
    (@__just_key Home) => { $crate::Key::Home };
    (@__just_key Insert) => { $crate::Key::Insert };
    (@__just_key Left) => { $crate::Key::Left };
    (@__just_key PageDown) => { $crate::Key::PageDown };
    (@__just_key PageUp) => { $crate::Key::PageUp };
    (@__just_key Right) => { $crate::Key::Right };
    (@__just_key Tab) => { $crate::Key::Tab };
    (@__just_key Up) => { $crate::Key::Up };
    (@__just_key Null) => { $crate::Key::Null };
    (@__just_key UnknownEscSeq) => { $crate::Key::UnknownEscSeq };
    (@__just_key BracketedPasteStart) => { $crate::Key::BracketedPasteStart };
    (@__just_key BracketedPasteEnd) => { $crate::Key::BracketedPasteEnd };
    (@__just_key Char($($c:tt)*)) => { $crate::Key::Char($($c)*) };
    (@__just_key F($($c:tt)*)) => { $crate::Key::F($($c)*) };

    (@__just_key $ch:literal) => { $crate::Key::Char($ch) };

    (@__just_key $ch:ident) => { compile_error!("`key_press!(<some mod>, c)` is ambiguous, try `key_press!(<some mod>, Char(c))`?") };

    (@__just_key $ch:pat) => { $crate::Key::Char($ch) };
}

#[cfg(any(windows, unix))]
pub fn char_to_key_press(c: char) -> KeyPress {
    if !c.is_control() {
        return c.into();
    }
    #[allow(clippy::match_same_arms)]
    match c {
        '\x00' => KeyPress::ctrl(' '),
        '\x01' => KeyPress::ctrl('A'),
        '\x02' => KeyPress::ctrl('B'),
        '\x03' => KeyPress::ctrl('C'),
        '\x04' => KeyPress::ctrl('D'),
        '\x05' => KeyPress::ctrl('E'),
        '\x06' => KeyPress::ctrl('F'),
        '\x07' => KeyPress::ctrl('G'),
        '\x08' => KeyPress::BACKSPACE, // '\b'
        '\x09' => KeyPress::TAB,       // '\t'
        '\x0a' => KeyPress::ctrl('J'), // '\n' (10)
        '\x0b' => KeyPress::ctrl('K'),
        '\x0c' => KeyPress::ctrl('L'),
        '\x0d' => KeyPress::ENTER, // '\r' (13)
        '\x0e' => KeyPress::ctrl('N'),
        '\x0f' => KeyPress::ctrl('O'),
        '\x10' => KeyPress::ctrl('P'),
        '\x12' => KeyPress::ctrl('R'),
        '\x13' => KeyPress::ctrl('S'),
        '\x14' => KeyPress::ctrl('T'),
        '\x15' => KeyPress::ctrl('U'),
        '\x16' => KeyPress::ctrl('V'),
        '\x17' => KeyPress::ctrl('W'),
        '\x18' => KeyPress::ctrl('X'),
        '\x19' => KeyPress::ctrl('Y'),
        '\x1a' => KeyPress::ctrl('Z'),
        '\x1b' => KeyPress::ESC, // Ctrl-[
        '\x1c' => KeyPress::ctrl('\\'),
        '\x1d' => KeyPress::ctrl(']'),
        '\x1e' => KeyPress::ctrl('^'),
        '\x1f' => KeyPress::ctrl('_'),
        '\x7f' => KeyPress::BACKSPACE, // Rubout
        _ => Key::Null.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::{char_to_key_press, Key, KeyMods, KeyPress};
    use assert_matches::assert_matches;
    #[test]
    fn char_to_key() {
        assert_eq!(KeyPress::ESC, char_to_key_press('\x1b'));
    }
    #[test]
    fn test_macro_keyidents() {
        assert_matches!(Key::Backspace.into(), key_press!(Backspace));
        assert_matches!(Key::BackTab.into(), key_press!(BackTab));
        assert_matches!(Key::Delete.into(), key_press!(Delete));
        assert_matches!(Key::Down.into(), key_press!(Down));
        assert_matches!(Key::End.into(), key_press!(End));
        assert_matches!(Key::Enter.into(), key_press!(Enter));
        assert_matches!(Key::Esc.into(), key_press!(Esc));
        assert_matches!(Key::Home.into(), key_press!(Home));
        assert_matches!(Key::Insert.into(), key_press!(Insert));
        assert_matches!(Key::Left.into(), key_press!(Left));
        assert_matches!(Key::PageDown.into(), key_press!(PageDown));
        assert_matches!(Key::PageUp.into(), key_press!(PageUp));
        assert_matches!(Key::Right.into(), key_press!(Right));
        assert_matches!(Key::Tab.into(), key_press!(Tab));
        assert_matches!(Key::Up.into(), key_press!(Up));
        assert_matches!(Key::Null.into(), key_press!(Null));
        assert_matches!(Key::UnknownEscSeq.into(), key_press!(UnknownEscSeq));

        assert_matches!(Key::Char('c').into(), key_press!(Char('c')));

        assert_matches!(Key::F(3).into(), key_press!(F(3)));
        assert_matches!(
            Key::BracketedPasteStart.into(),
            key_press!(BracketedPasteStart)
        );
        assert_matches!(Key::BracketedPasteEnd.into(), key_press!(BracketedPasteEnd));
    }

    #[test]
    fn test_macro_patterns() {
        assert_matches!(KeyPress::from('x'), key_press!(Char(_c)));
        assert_matches!(KeyPress::from('x'), key_press!(Char(..)));
        assert_matches!(KeyPress::from('x'), key_press!(Char('a'..='z')));
        assert_matches!(KeyPress::from('x'), key_press!('a'..='z'));
        assert_matches!(KeyPress::from('x'), key_press!('x'));
        assert_matches!(KeyPress::from('x'), key_press!(letter @ 'a' ..= 'z') if letter == 'x');
    }

    #[test]
    fn test_macro_mods() {
        assert_matches!(
            KeyPress::new('x'.into(), KeyMods::SHIFT),
            key_press!(SHIFT, 'x')
        );
        assert_matches!(
            KeyPress::new('x'.into(), KeyMods::CTRL),
            key_press!(CTRL, 'x')
        );
        assert_matches!(
            KeyPress::new('x'.into(), KeyMods::META),
            key_press!(META, 'x')
        );
        assert_matches!(
            KeyPress::new('x'.into(), KeyMods::NONE),
            key_press!(NONE, 'x')
        );

        assert_matches!(
            KeyPress::new('x'.into(), KeyMods::CTRL_SHIFT),
            key_press!(CTRL_SHIFT, 'x')
        );
        assert_matches!(
            KeyPress::new('x'.into(), KeyMods::META_SHIFT),
            key_press!(META_SHIFT, 'x')
        );
        assert_matches!(
            KeyPress::new('x'.into(), KeyMods::CTRL_META),
            key_press!(CTRL_META, 'x')
        );
        assert_matches!(
            KeyPress::new('x'.into(), KeyMods::CTRL_META_SHIFT),
            key_press!(CTRL_META_SHIFT, 'x')
        );
    }
    #[test]
    fn test_macro_mod_key_idents() {
        assert_matches!(Key::Backspace.into(), key_press!(NONE, Backspace));
        assert_matches!(Key::BackTab.into(), key_press!(NONE, BackTab));
        assert_matches!(Key::Delete.into(), key_press!(NONE, Delete));
        assert_matches!(Key::Down.into(), key_press!(NONE, Down));
        assert_matches!(Key::End.into(), key_press!(NONE, End));
        assert_matches!(Key::Enter.into(), key_press!(NONE, Enter));
        assert_matches!(Key::Esc.into(), key_press!(NONE, Esc));
        assert_matches!(Key::Home.into(), key_press!(NONE, Home));
        assert_matches!(Key::Insert.into(), key_press!(NONE, Insert));
        assert_matches!(Key::Left.into(), key_press!(NONE, Left));
        assert_matches!(Key::PageDown.into(), key_press!(NONE, PageDown));
        assert_matches!(Key::PageUp.into(), key_press!(NONE, PageUp));
        assert_matches!(Key::Right.into(), key_press!(NONE, Right));
        assert_matches!(Key::Tab.into(), key_press!(NONE, Tab));
        assert_matches!(Key::Up.into(), key_press!(NONE, Up));
        assert_matches!(Key::Null.into(), key_press!(NONE, Null));
        assert_matches!(Key::UnknownEscSeq.into(), key_press!(NONE, UnknownEscSeq));

        assert_matches!(Key::Char('c').into(), key_press!(NONE, Char('c')));

        assert_matches!(Key::F(3).into(), key_press!(NONE, F(3)));
        assert_matches!(
            Key::BracketedPasteStart.into(),
            key_press!(NONE, BracketedPasteStart)
        );
    }
}
