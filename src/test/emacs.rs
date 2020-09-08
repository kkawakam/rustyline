//! Emacs specific key bindings
use super::{assert_cursor, assert_history};
use crate::config::EditMode;
use crate::keys::{KeyCode as K, Modifiers as M};

#[test]
fn ctrl_a() {
    assert_cursor(
        EditMode::Emacs,
        ("Hi", ""),
        &[(K::Char('A'), M::CTRL), (K::Enter, M::NONE)],
        ("", "Hi"),
    );
    assert_cursor(
        EditMode::Emacs,
        ("test test\n123", "foo"),
        &[(K::Char('A'), M::CTRL), (K::Enter, M::NONE)],
        ("test test\n", "123foo"),
    );
}

#[test]
fn ctrl_e() {
    assert_cursor(
        EditMode::Emacs,
        ("", "Hi"),
        &[(K::Char('E'), M::CTRL), (K::Enter, M::NONE)],
        ("Hi", ""),
    );
    assert_cursor(
        EditMode::Emacs,
        ("foo", "test test\n123"),
        &[(K::Char('E'), M::CTRL), (K::Enter, M::NONE)],
        ("footest test", "\n123"),
    );
}

#[test]
fn ctrl_b() {
    assert_cursor(
        EditMode::Emacs,
        ("Hi", ""),
        &[(K::Char('B'), M::CTRL), (K::Enter, M::NONE)],
        ("H", "i"),
    );
    assert_cursor(
        EditMode::Emacs,
        ("Hi", ""),
        &[
            (K::Char('2'), M::ALT),
            (K::Char('B'), M::CTRL),
            (K::Enter, M::NONE),
        ],
        ("", "Hi"),
    );
    assert_cursor(
        EditMode::Emacs,
        ("", "Hi"),
        &[
            (K::Char('-'), M::ALT),
            (K::Char('2'), M::ALT),
            (K::Char('B'), M::CTRL),
            (K::Enter, M::NONE),
        ],
        ("Hi", ""),
    );
}

#[test]
fn ctrl_f() {
    assert_cursor(
        EditMode::Emacs,
        ("", "Hi"),
        &[(K::Char('F'), M::CTRL), (K::Enter, M::NONE)],
        ("H", "i"),
    );
    assert_cursor(
        EditMode::Emacs,
        ("", "Hi"),
        &[
            (K::Char('2'), M::ALT),
            (K::Char('F'), M::CTRL),
            (K::Enter, M::NONE),
        ],
        ("Hi", ""),
    );
    assert_cursor(
        EditMode::Emacs,
        ("Hi", ""),
        &[
            (K::Char('-'), M::ALT),
            (K::Char('2'), M::ALT),
            (K::Char('F'), M::CTRL),
            (K::Enter, M::NONE),
        ],
        ("", "Hi"),
    );
}

#[test]
fn ctrl_h() {
    assert_cursor(
        EditMode::Emacs,
        ("Hi", ""),
        &[(K::Char('H'), M::CTRL), (K::Enter, M::NONE)],
        ("H", ""),
    );
    assert_cursor(
        EditMode::Emacs,
        ("Hi", ""),
        &[
            (K::Char('2'), M::ALT),
            (K::Char('H'), M::CTRL),
            (K::Enter, M::NONE),
        ],
        ("", ""),
    );
    assert_cursor(
        EditMode::Emacs,
        ("", "Hi"),
        &[
            (K::Char('-'), M::ALT),
            (K::Char('2'), M::ALT),
            (K::Char('H'), M::CTRL),
            (K::Enter, M::NONE),
        ],
        ("", ""),
    );
}

#[test]
fn backspace() {
    assert_cursor(
        EditMode::Emacs,
        ("", ""),
        &[(K::Backspace, M::NONE), (K::Enter, M::NONE)],
        ("", ""),
    );
    assert_cursor(
        EditMode::Emacs,
        ("Hi", ""),
        &[(K::Backspace, M::NONE), (K::Enter, M::NONE)],
        ("H", ""),
    );
    assert_cursor(
        EditMode::Emacs,
        ("", "Hi"),
        &[(K::Backspace, M::NONE), (K::Enter, M::NONE)],
        ("", "Hi"),
    );
}

#[test]
fn ctrl_k() {
    assert_cursor(
        EditMode::Emacs,
        ("Hi", ""),
        &[(K::Char('K'), M::CTRL), (K::Enter, M::NONE)],
        ("Hi", ""),
    );
    assert_cursor(
        EditMode::Emacs,
        ("", "Hi"),
        &[(K::Char('K'), M::CTRL), (K::Enter, M::NONE)],
        ("", ""),
    );
    assert_cursor(
        EditMode::Emacs,
        ("B", "ye"),
        &[(K::Char('K'), M::CTRL), (K::Enter, M::NONE)],
        ("B", ""),
    );
    assert_cursor(
        EditMode::Emacs,
        ("Hi", "foo\nbar"),
        &[(K::Char('K'), M::CTRL), (K::Enter, M::NONE)],
        ("Hi", "\nbar"),
    );
    assert_cursor(
        EditMode::Emacs,
        ("Hi", "\nbar"),
        &[(K::Char('K'), M::CTRL), (K::Enter, M::NONE)],
        ("Hi", "bar"),
    );
    assert_cursor(
        EditMode::Emacs,
        ("Hi", "bar"),
        &[(K::Char('K'), M::CTRL), (K::Enter, M::NONE)],
        ("Hi", ""),
    );
}

#[test]
fn ctrl_u() {
    assert_cursor(
        EditMode::Emacs,
        ("", "Hi"),
        &[(K::Char('U'), M::CTRL), (K::Enter, M::NONE)],
        ("", "Hi"),
    );
    assert_cursor(
        EditMode::Emacs,
        ("Hi", ""),
        &[(K::Char('U'), M::CTRL), (K::Enter, M::NONE)],
        ("", ""),
    );
    assert_cursor(
        EditMode::Emacs,
        ("B", "ye"),
        &[(K::Char('U'), M::CTRL), (K::Enter, M::NONE)],
        ("", "ye"),
    );
    assert_cursor(
        EditMode::Emacs,
        ("foo\nbar", "Hi"),
        &[(K::Char('U'), M::CTRL), (K::Enter, M::NONE)],
        ("foo\n", "Hi"),
    );
    assert_cursor(
        EditMode::Emacs,
        ("foo\n", "Hi"),
        &[(K::Char('U'), M::CTRL), (K::Enter, M::NONE)],
        ("foo", "Hi"),
    );
    assert_cursor(
        EditMode::Emacs,
        ("foo", "Hi"),
        &[(K::Char('U'), M::CTRL), (K::Enter, M::NONE)],
        ("", "Hi"),
    );
}
#[test]
fn ctrl_n() {
    assert_history(
        EditMode::Emacs,
        &["line1", "line2"],
        &[
            (K::Char('P'), M::CTRL),
            (K::Char('P'), M::CTRL),
            (K::Char('N'), M::CTRL),
            (K::Enter, M::NONE),
        ],
        "",
        ("line2", ""),
    );
}

#[test]
fn ctrl_p() {
    assert_history(
        EditMode::Emacs,
        &["line1"],
        &[(K::Char('P'), M::CTRL), (K::Enter, M::NONE)],
        "",
        ("line1", ""),
    );
}

#[test]
fn ctrl_t() {
    /* FIXME
    assert_cursor(
        ("ab", "cd"),
        &[(K::Char('2'), M::ALT), (K::Char('T'), M::CTRL), (K::Enter, M::NONE)],
        ("acdb", ""),
    );*/
}

#[test]
fn ctrl_x_ctrl_u() {
    assert_cursor(
        EditMode::Emacs,
        ("Hello, ", "world"),
        &[
            (K::Char('W'), M::CTRL),
            (K::Char('X'), M::CTRL),
            (K::Char('U'), M::CTRL),
            (K::Enter, M::NONE),
        ],
        ("Hello, ", "world"),
    );
}

#[test]
fn meta_b() {
    assert_cursor(
        EditMode::Emacs,
        ("Hello, world!", ""),
        &[(K::Char('B'), M::ALT), (K::Enter, M::NONE)],
        ("Hello, ", "world!"),
    );
    assert_cursor(
        EditMode::Emacs,
        ("Hello, world!", ""),
        &[
            (K::Char('2'), M::ALT),
            (K::Char('B'), M::ALT),
            (K::Enter, M::NONE),
        ],
        ("", "Hello, world!"),
    );
    assert_cursor(
        EditMode::Emacs,
        ("", "Hello, world!"),
        &[
            (K::Char('-'), M::ALT),
            (K::Char('B'), M::ALT),
            (K::Enter, M::NONE),
        ],
        ("Hello", ", world!"),
    );
}

#[test]
fn meta_f() {
    assert_cursor(
        EditMode::Emacs,
        ("", "Hello, world!"),
        &[(K::Char('F'), M::ALT), (K::Enter, M::NONE)],
        ("Hello", ", world!"),
    );
    assert_cursor(
        EditMode::Emacs,
        ("", "Hello, world!"),
        &[
            (K::Char('2'), M::ALT),
            (K::Char('F'), M::ALT),
            (K::Enter, M::NONE),
        ],
        ("Hello, world", "!"),
    );
    assert_cursor(
        EditMode::Emacs,
        ("Hello, world!", ""),
        &[
            (K::Char('-'), M::ALT),
            (K::Char('F'), M::ALT),
            (K::Enter, M::NONE),
        ],
        ("Hello, ", "world!"),
    );
}

#[test]
fn meta_c() {
    assert_cursor(
        EditMode::Emacs,
        ("hi", ""),
        &[(K::Char('C'), M::ALT), (K::Enter, M::NONE)],
        ("hi", ""),
    );
    assert_cursor(
        EditMode::Emacs,
        ("", "hi"),
        &[(K::Char('C'), M::ALT), (K::Enter, M::NONE)],
        ("Hi", ""),
    );
    /* FIXME
    assert_cursor(
        ("", "hi test"),
        &[(K::Char('2'), M::ALT), (K::Char('C'), M::ALT), (K::Enter, M::NONE)],
        ("Hi Test", ""),
    );*/
}

#[test]
fn meta_l() {
    assert_cursor(
        EditMode::Emacs,
        ("Hi", ""),
        &[(K::Char('L'), M::ALT), (K::Enter, M::NONE)],
        ("Hi", ""),
    );
    assert_cursor(
        EditMode::Emacs,
        ("", "HI"),
        &[(K::Char('L'), M::ALT), (K::Enter, M::NONE)],
        ("hi", ""),
    );
    /* FIXME
    assert_cursor(
        ("", "HI TEST"),
        &[(K::Char('2'), M::ALT), (K::Char('L'), M::ALT), (K::Enter, M::NONE)],
        ("hi test", ""),
    );*/
}

#[test]
fn meta_u() {
    assert_cursor(
        EditMode::Emacs,
        ("hi", ""),
        &[(K::Char('U'), M::ALT), (K::Enter, M::NONE)],
        ("hi", ""),
    );
    assert_cursor(
        EditMode::Emacs,
        ("", "hi"),
        &[(K::Char('U'), M::ALT), (K::Enter, M::NONE)],
        ("HI", ""),
    );
    /* FIXME
    assert_cursor(
        ("", "hi test"),
        &[(K::Char('2'), M::ALT), (K::Char('U'), M::ALT), (K::Enter, M::NONE)],
        ("HI TEST", ""),
    );*/
}

#[test]
fn meta_d() {
    assert_cursor(
        EditMode::Emacs,
        ("Hello", ", world!"),
        &[(K::Char('D'), M::ALT), (K::Enter, M::NONE)],
        ("Hello", "!"),
    );
    assert_cursor(
        EditMode::Emacs,
        ("Hello", ", world!"),
        &[
            (K::Char('2'), M::ALT),
            (K::Char('D'), M::ALT),
            (K::Enter, M::NONE),
        ],
        ("Hello", ""),
    );
}

#[test]
fn meta_t() {
    assert_cursor(
        EditMode::Emacs,
        ("Hello", ", world!"),
        &[(K::Char('T'), M::ALT), (K::Enter, M::NONE)],
        ("world, Hello", "!"),
    );
    /* FIXME
    assert_cursor(
        ("One Two", " Three Four"),
        &[(K::Char('T'), M::ALT), (K::Enter, M::NONE)],
        ("One Four Three Two", ""),
    );*/
}

#[test]
fn meta_y() {
    assert_cursor(
        EditMode::Emacs,
        ("Hello, world", "!"),
        &[
            (K::Char('W'), M::CTRL),
            (K::Left, M::NONE),
            (K::Char('W'), M::CTRL),
            (K::Char('Y'), M::CTRL),
            (K::Char('Y'), M::ALT),
            (K::Enter, M::NONE),
        ],
        ("world", " !"),
    );
}

#[test]
fn meta_backspace() {
    assert_cursor(
        EditMode::Emacs,
        ("Hello, wor", "ld!"),
        &[(K::Char('\x08'), M::ALT), (K::Enter, M::NONE)],
        ("Hello, ", "ld!"),
    );
}

#[test]
fn meta_digit() {
    assert_cursor(
        EditMode::Emacs,
        ("", ""),
        &[
            (K::Char('3'), M::ALT),
            (K::Char('h'), M::NONE),
            (K::Enter, M::NONE),
        ],
        ("hhh", ""),
    );
}
