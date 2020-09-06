//! Vi command mode specific key bindings
use super::{assert_cursor, assert_history};
use crate::config::EditMode;
use crate::keys::{KeyCode as K, Modifiers as M};

#[test]
fn dollar() {
    assert_cursor(
        EditMode::Vi,
        ("", "Hi"),
        &[(K::Esc, M::NONE), (K::Char('$'), M::NONE), (K::Enter, M::NONE)],
        ("Hi", ""), // FIXME
    );
}

/*#[test]
fn dot() {
    // TODO
}*/

#[test]
fn semi_colon() {
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[
            (K::Esc, M::NONE),
            (K::Char('f'), M::NONE),
            (K::Char('o'), M::NONE),
            (K::Char(';'), M::NONE),
            (K::Enter, M::NONE),
        ],
        ("Hello, w", "orld!"),
    );
}

#[test]
fn comma() {
    assert_cursor(
        EditMode::Vi,
        ("Hello, w", "orld!"),
        &[
            (K::Esc, M::NONE),
            (K::Char('f'), M::NONE),
            (K::Char('l'), M::NONE),
            (K::Char(','), M::NONE),
            (K::Enter, M::NONE),
        ],
        ("Hel", "lo, world!"),
    );
}

#[test]
fn zero() {
    assert_cursor(
        EditMode::Vi,
        ("Hi", ""),
        &[(K::Esc, M::NONE), (K::Char('0'), M::NONE), (K::Enter, M::NONE)],
        ("", "Hi"),
    );
}

#[test]
fn caret() {
    assert_cursor(
        EditMode::Vi,
        (" Hi", ""),
        &[(K::Esc, M::NONE), (K::Char('^'), M::NONE), (K::Enter, M::NONE)],
        (" ", "Hi"),
    );
}

#[test]
fn a() {
    assert_cursor(
        EditMode::Vi,
        ("B", "e"),
        &[
            (K::Esc, M::NONE),
            (K::Char('a'), M::NONE),
            (K::Char('y'), M::NONE),
            (K::Enter, M::NONE),
        ],
        ("By", "e"),
    );
}

#[test]
fn uppercase_a() {
    assert_cursor(
        EditMode::Vi,
        ("", "By"),
        &[
            (K::Esc, M::NONE),
            (K::Char('A'), M::NONE),
            (K::Char('e'), M::NONE),
            (K::Enter, M::NONE),
        ],
        ("Bye", ""),
    );
}

#[test]
fn b() {
    assert_cursor(
        EditMode::Vi,
        ("Hello, world!", ""),
        &[(K::Esc, M::NONE), (K::Char('b'), M::NONE), (K::Enter, M::NONE)],
        ("Hello, ", "world!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("Hello, world!", ""),
        &[
            (K::Esc, M::NONE),
            (K::Char('2'), M::NONE),
            (K::Char('b'), M::NONE),
            (K::Enter, M::NONE),
        ],
        ("Hello", ", world!"),
    );
}

#[test]
fn uppercase_b() {
    assert_cursor(
        EditMode::Vi,
        ("Hello, world!", ""),
        &[(K::Esc, M::NONE), (K::Char('B'), M::NONE), (K::Enter, M::NONE)],
        ("Hello, ", "world!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("Hello, world!", ""),
        &[
            (K::Esc, M::NONE),
            (K::Char('2'), M::NONE),
            (K::Char('B'), M::NONE),
            (K::Enter, M::NONE),
        ],
        ("", "Hello, world!"),
    );
}

#[test]
fn uppercase_c() {
    assert_cursor(
        EditMode::Vi,
        ("Hello, w", "orld!"),
        &[
            (K::Esc, M::NONE),
            (K::Char('C'), M::NONE),
            (K::Char('i'), M::NONE),
            (K::Enter, M::NONE),
        ],
        ("Hello, i", ""),
    );
}

#[test]
fn ctrl_k() {
    for key in &[(K::Char('D'), M::NONE), (K::Char('K'), M::CTRL)] {
        assert_cursor(
            EditMode::Vi,
            ("Hi", ""),
            &[(K::Esc, M::NONE), *key, (K::Enter, M::NONE)],
            ("H", ""),
        );
        assert_cursor(
            EditMode::Vi,
            ("", "Hi"),
            &[(K::Esc, M::NONE), *key, (K::Enter, M::NONE)],
            ("", ""),
        );
        assert_cursor(
            EditMode::Vi,
            ("By", "e"),
            &[(K::Esc, M::NONE), *key, (K::Enter, M::NONE)],
            ("B", ""),
        );
    }
}

#[test]
fn e() {
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[(K::Esc, M::NONE), (K::Char('e'), M::NONE), (K::Enter, M::NONE)],
        ("Hell", "o, world!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[
            (K::Esc, M::NONE),
            (K::Char('2'), M::NONE),
            (K::Char('e'), M::NONE),
            (K::Enter, M::NONE),
        ],
        ("Hello, worl", "d!"),
    );
}

#[test]
fn uppercase_e() {
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[(K::Esc, M::NONE), (K::Char('E'), M::NONE), (K::Enter, M::NONE)],
        ("Hello", ", world!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[
            (K::Esc, M::NONE),
            (K::Char('2'), M::NONE),
            (K::Char('E'), M::NONE),
            (K::Enter, M::NONE),
        ],
        ("Hello, world", "!"),
    );
}

#[test]
fn f() {
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[
            (K::Esc, M::NONE),
            (K::Char('f'), M::NONE),
            (K::Char('r'), M::NONE),
            (K::Enter, M::NONE),
        ],
        ("Hello, wo", "rld!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[
            (K::Esc, M::NONE),
            (K::Char('3'), M::NONE),
            (K::Char('f'), M::NONE),
            (K::Char('l'), M::NONE),
            (K::Enter, M::NONE),
        ],
        ("Hello, wor", "ld!"),
    );
}

#[test]
fn uppercase_f() {
    assert_cursor(
        EditMode::Vi,
        ("Hello, world!", ""),
        &[
            (K::Esc, M::NONE),
            (K::Char('F'), M::NONE),
            (K::Char('r'), M::NONE),
            (K::Enter, M::NONE),
        ],
        ("Hello, wo", "rld!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("Hello, world!", ""),
        &[
            (K::Esc, M::NONE),
            (K::Char('3'), M::NONE),
            (K::Char('F'), M::NONE),
            (K::Char('l'), M::NONE),
            (K::Enter, M::NONE),
        ],
        ("He", "llo, world!"),
    );
}

#[test]
fn i() {
    assert_cursor(
        EditMode::Vi,
        ("Be", ""),
        &[
            (K::Esc, M::NONE),
            (K::Char('i'), M::NONE),
            (K::Char('y'), M::NONE),
            (K::Enter, M::NONE),
        ],
        ("By", "e"),
    );
}

#[test]
fn uppercase_i() {
    assert_cursor(
        EditMode::Vi,
        ("Be", ""),
        &[
            (K::Esc, M::NONE),
            (K::Char('I'), M::NONE),
            (K::Char('y'), M::NONE),
            (K::Enter, M::NONE),
        ],
        ("y", "Be"),
    );
}

#[test]
fn u() {
    assert_cursor(
        EditMode::Vi,
        ("Hello, ", "world"),
        &[
            (K::Esc, M::NONE),
            (K::Char('W'), M::CTRL),
            (K::Char('u'), M::NONE),
            (K::Enter, M::NONE),
        ],
        ("Hello,", " world"),
    );
}

#[test]
fn w() {
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[(K::Esc, M::NONE), (K::Char('w'), M::NONE), (K::Enter, M::NONE)],
        ("Hello", ", world!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[
            (K::Esc, M::NONE),
            (K::Char('2'), M::NONE),
            (K::Char('w'), M::NONE),
            (K::Enter, M::NONE),
        ],
        ("Hello, ", "world!"),
    );
}

#[test]
fn uppercase_w() {
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[(K::Esc, M::NONE), (K::Char('W'), M::NONE), (K::Enter, M::NONE)],
        ("Hello, ", "world!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[
            (K::Esc, M::NONE),
            (K::Char('2'), M::NONE),
            (K::Char('W'), M::NONE),
            (K::Enter, M::NONE),
        ],
        ("Hello, world", "!"),
    );
}

#[test]
fn x() {
    assert_cursor(
        EditMode::Vi,
        ("", "a"),
        &[(K::Esc, M::NONE), (K::Char('x'), M::NONE), (K::Enter, M::NONE)],
        ("", ""),
    );
}

#[test]
fn uppercase_x() {
    assert_cursor(
        EditMode::Vi,
        ("Hi", ""),
        &[(K::Esc, M::NONE), (K::Char('X'), M::NONE), (K::Enter, M::NONE)],
        ("", "i"),
    );
}

#[test]
fn h() {
    for key in &[
        (K::Char('h'), M::NONE),
        (K::Char('H'), M::CTRL),
        (K::Backspace, M::NONE),
    ] {
        assert_cursor(
            EditMode::Vi,
            ("Bye", ""),
            &[(K::Esc, M::NONE), *key, (K::Enter, M::NONE)],
            ("B", "ye"),
        );
        assert_cursor(
            EditMode::Vi,
            ("Bye", ""),
            &[(K::Esc, M::NONE), (K::Char('2'), M::NONE), *key, (K::Enter, M::NONE)],
            ("", "Bye"),
        );
    }
}

#[test]
fn l() {
    for key in &[(K::Char('l'), M::NONE), (K::Char(' '), M::NONE)] {
        assert_cursor(
            EditMode::Vi,
            ("", "Hi"),
            &[(K::Esc, M::NONE), *key, (K::Enter, M::NONE)],
            ("H", "i"),
        );
        assert_cursor(
            EditMode::Vi,
            ("", "Hi"),
            &[(K::Esc, M::NONE), (K::Char('2'), M::NONE), *key, (K::Enter, M::NONE)],
            ("Hi", ""),
        );
    }
}

#[test]
fn j() {
    for key in &[(K::Char('j'), M::NONE), (K::Char('+'), M::NONE)] {
        assert_cursor(
            EditMode::Vi,
            ("Hel", "lo,\nworld!"),
            // NOTE: escape moves backwards on char
            &[(K::Esc, M::NONE), *key, (K::Enter, M::NONE)],
            ("Hello,\nwo", "rld!"),
        );
        assert_cursor(
            EditMode::Vi,
            ("", "One\nTwo\nThree"),
            &[(K::Esc, M::NONE), (K::Char('2'), M::NONE), *key, (K::Enter, M::NONE)],
            ("One\nTwo\n", "Three"),
        );
        assert_cursor(
            EditMode::Vi,
            ("Hel", "lo,\nworld!"),
            // NOTE: escape moves backwards on char
            &[(K::Esc, M::NONE), (K::Char('7'), M::NONE), *key, (K::Enter, M::NONE)],
            ("Hello,\nwo", "rld!"),
        );
    }
}

#[test]
fn k() {
    for key in &[(K::Char('k'), M::NONE), (K::Char('-'), M::NONE)] {
        assert_cursor(
            EditMode::Vi,
            ("Hello,\nworl", "d!"),
            // NOTE: escape moves backwards on char
            &[(K::Esc, M::NONE), *key, (K::Enter, M::NONE)],
            ("Hel", "lo,\nworld!"),
        );
        assert_cursor(
            EditMode::Vi,
            ("One\nTwo\nT", "hree"),
            // NOTE: escape moves backwards on char
            &[(K::Esc, M::NONE), (K::Char('2'), M::NONE), *key, (K::Enter, M::NONE)],
            ("", "One\nTwo\nThree"),
        );
        assert_cursor(
            EditMode::Vi,
            ("Hello,\nworl", "d!"),
            // NOTE: escape moves backwards on char
            &[(K::Esc, M::NONE), (K::Char('5'), M::NONE), *key, (K::Enter, M::NONE)],
            ("Hel", "lo,\nworld!"),
        );
        assert_cursor(
            EditMode::Vi,
            ("first line\nshort\nlong line", ""),
            &[(K::Esc, M::NONE), *key, (K::Enter, M::NONE)],
            ("first line\nshort", "\nlong line"),
        );
    }
}

#[test]
fn ctrl_n() {
    for key in &[(K::Char('N'), M::CTRL)] {
        assert_history(
            EditMode::Vi,
            &["line1", "line2"],
            &[
                (K::Esc, M::NONE),
                (K::Char('P'), M::CTRL),
                (K::Char('P'), M::CTRL),
                *key,
                (K::Enter, M::NONE),
            ],
            "",
            ("line2", ""),
        );
    }
}

#[test]
fn ctrl_p() {
    for key in &[(K::Char('P'), M::CTRL)] {
        assert_history(
            EditMode::Vi,
            &["line1"],
            &[(K::Esc, M::NONE), *key, (K::Enter, M::NONE)],
            "",
            ("line1", ""),
        );
    }
}

#[test]
fn p() {
    assert_cursor(
        EditMode::Vi,
        ("Hello, ", "world"),
        &[
            (K::Esc, M::NONE),
            (K::Char('W'), M::CTRL),
            (K::Char('p'), M::NONE),
            (K::Enter, M::NONE),
        ],
        (" Hello", ",world"),
    );
}

#[test]
fn uppercase_p() {
    assert_cursor(
        EditMode::Vi,
        ("Hello, ", "world"),
        &[
            (K::Esc, M::NONE),
            (K::Char('W'), M::CTRL),
            (K::Char('P'), M::NONE),
            (K::Enter, M::NONE),
        ],
        ("Hello", ", world"),
    );
}

#[test]
fn r() {
    assert_cursor(
        EditMode::Vi,
        ("Hi", ", world!"),
        &[
            (K::Esc, M::NONE),
            (K::Char('r'), M::NONE),
            (K::Char('o'), M::NONE),
            (K::Enter, M::NONE),
        ],
        ("H", "o, world!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("He", "llo, world!"),
        &[
            (K::Esc, M::NONE),
            (K::Char('4'), M::NONE),
            (K::Char('r'), M::NONE),
            (K::Char('i'), M::NONE),
            (K::Enter, M::NONE),
        ],
        ("Hiii", "i, world!"),
    );
}

#[test]
fn s() {
    assert_cursor(
        EditMode::Vi,
        ("Hi", ", world!"),
        &[
            (K::Esc, M::NONE),
            (K::Char('s'), M::NONE),
            (K::Char('o'), M::NONE),
            (K::Enter, M::NONE),
        ],
        ("Ho", ", world!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("He", "llo, world!"),
        &[
            (K::Esc, M::NONE),
            (K::Char('4'), M::NONE),
            (K::Char('s'), M::NONE),
            (K::Char('i'), M::NONE),
            (K::Enter, M::NONE),
        ],
        ("Hi", ", world!"),
    );
}

#[test]
fn uppercase_s() {
    assert_cursor(
        EditMode::Vi,
        ("Hello, ", "world"),
        &[(K::Esc, M::NONE), (K::Char('S'), M::NONE), (K::Enter, M::NONE)],
        ("", ""),
    );
}

#[test]
fn t() {
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[
            (K::Esc, M::NONE),
            (K::Char('t'), M::NONE),
            (K::Char('r'), M::NONE),
            (K::Enter, M::NONE),
        ],
        ("Hello, w", "orld!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[
            (K::Esc, M::NONE),
            (K::Char('3'), M::NONE),
            (K::Char('t'), M::NONE),
            (K::Char('l'), M::NONE),
            (K::Enter, M::NONE),
        ],
        ("Hello, wo", "rld!"),
    );
}

#[test]
fn uppercase_t() {
    assert_cursor(
        EditMode::Vi,
        ("Hello, world!", ""),
        &[
            (K::Esc, M::NONE),
            (K::Char('T'), M::NONE),
            (K::Char('r'), M::NONE),
            (K::Enter, M::NONE),
        ],
        ("Hello, wor", "ld!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("Hello, world!", ""),
        &[
            (K::Esc, M::NONE),
            (K::Char('3'), M::NONE),
            (K::Char('T'), M::NONE),
            (K::Char('l'), M::NONE),
            (K::Enter, M::NONE),
        ],
        ("Hel", "lo, world!"),
    );
}
