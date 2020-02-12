//! Vi command mode specific key bindings
use super::{assert_cursor, assert_history};
use crate::config::EditMode;
use crate::keys::KeyPress;

#[test]
fn dollar() {
    assert_cursor(
        EditMode::Vi,
        ("", "Hi"),
        &[KeyPress::ESC, KeyPress::from('$'), KeyPress::ENTER],
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
            KeyPress::ESC,
            KeyPress::from('f'),
            KeyPress::from('o'),
            KeyPress::from(';'),
            KeyPress::ENTER,
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
            KeyPress::ESC,
            KeyPress::from('f'),
            KeyPress::from('l'),
            KeyPress::from(','),
            KeyPress::ENTER,
        ],
        ("Hel", "lo, world!"),
    );
}

#[test]
fn zero() {
    assert_cursor(
        EditMode::Vi,
        ("Hi", ""),
        &[KeyPress::ESC, KeyPress::from('0'), KeyPress::ENTER],
        ("", "Hi"),
    );
}

#[test]
fn caret() {
    assert_cursor(
        EditMode::Vi,
        (" Hi", ""),
        &[KeyPress::ESC, KeyPress::from('^'), KeyPress::ENTER],
        (" ", "Hi"),
    );
}

#[test]
fn a() {
    assert_cursor(
        EditMode::Vi,
        ("B", "e"),
        &[
            KeyPress::ESC,
            KeyPress::from('a'),
            KeyPress::from('y'),
            KeyPress::ENTER,
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
            KeyPress::ESC,
            KeyPress::from('A'),
            KeyPress::from('e'),
            KeyPress::ENTER,
        ],
        ("Bye", ""),
    );
}

#[test]
fn b() {
    assert_cursor(
        EditMode::Vi,
        ("Hello, world!", ""),
        &[KeyPress::ESC, KeyPress::from('b'), KeyPress::ENTER],
        ("Hello, ", "world!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("Hello, world!", ""),
        &[
            KeyPress::ESC,
            KeyPress::from('2'),
            KeyPress::from('b'),
            KeyPress::ENTER,
        ],
        ("Hello", ", world!"),
    );
}

#[test]
fn uppercase_b() {
    assert_cursor(
        EditMode::Vi,
        ("Hello, world!", ""),
        &[KeyPress::ESC, KeyPress::from('B'), KeyPress::ENTER],
        ("Hello, ", "world!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("Hello, world!", ""),
        &[
            KeyPress::ESC,
            KeyPress::from('2'),
            KeyPress::from('B'),
            KeyPress::ENTER,
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
            KeyPress::ESC,
            KeyPress::from('C'),
            KeyPress::from('i'),
            KeyPress::ENTER,
        ],
        ("Hello, i", ""),
    );
}

#[test]
fn ctrl_k() {
    for key in &[KeyPress::from('D'), KeyPress::ctrl('K')] {
        assert_cursor(
            EditMode::Vi,
            ("Hi", ""),
            &[KeyPress::ESC, *key, KeyPress::ENTER],
            ("H", ""),
        );
        assert_cursor(
            EditMode::Vi,
            ("", "Hi"),
            &[KeyPress::ESC, *key, KeyPress::ENTER],
            ("", ""),
        );
        assert_cursor(
            EditMode::Vi,
            ("By", "e"),
            &[KeyPress::ESC, *key, KeyPress::ENTER],
            ("B", ""),
        );
    }
}

#[test]
fn e() {
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[KeyPress::ESC, KeyPress::from('e'), KeyPress::ENTER],
        ("Hell", "o, world!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[
            KeyPress::ESC,
            KeyPress::from('2'),
            KeyPress::from('e'),
            KeyPress::ENTER,
        ],
        ("Hello, worl", "d!"),
    );
}

#[test]
fn uppercase_e() {
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[KeyPress::ESC, KeyPress::from('E'), KeyPress::ENTER],
        ("Hello", ", world!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[
            KeyPress::ESC,
            KeyPress::from('2'),
            KeyPress::from('E'),
            KeyPress::ENTER,
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
            KeyPress::ESC,
            KeyPress::from('f'),
            KeyPress::from('r'),
            KeyPress::ENTER,
        ],
        ("Hello, wo", "rld!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[
            KeyPress::ESC,
            KeyPress::from('3'),
            KeyPress::from('f'),
            KeyPress::from('l'),
            KeyPress::ENTER,
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
            KeyPress::ESC,
            KeyPress::from('F'),
            KeyPress::from('r'),
            KeyPress::ENTER,
        ],
        ("Hello, wo", "rld!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("Hello, world!", ""),
        &[
            KeyPress::ESC,
            KeyPress::from('3'),
            KeyPress::from('F'),
            KeyPress::from('l'),
            KeyPress::ENTER,
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
            KeyPress::ESC,
            KeyPress::from('i'),
            KeyPress::from('y'),
            KeyPress::ENTER,
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
            KeyPress::ESC,
            KeyPress::from('I'),
            KeyPress::from('y'),
            KeyPress::ENTER,
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
            KeyPress::ESC,
            KeyPress::ctrl('W'),
            KeyPress::from('u'),
            KeyPress::ENTER,
        ],
        ("Hello,", " world"),
    );
}

#[test]
fn w() {
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[KeyPress::ESC, KeyPress::from('w'), KeyPress::ENTER],
        ("Hello", ", world!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[
            KeyPress::ESC,
            KeyPress::from('2'),
            KeyPress::from('w'),
            KeyPress::ENTER,
        ],
        ("Hello, ", "world!"),
    );
}

#[test]
fn uppercase_w() {
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[KeyPress::ESC, KeyPress::from('W'), KeyPress::ENTER],
        ("Hello, ", "world!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[
            KeyPress::ESC,
            KeyPress::from('2'),
            KeyPress::from('W'),
            KeyPress::ENTER,
        ],
        ("Hello, world", "!"),
    );
}

#[test]
fn x() {
    assert_cursor(
        EditMode::Vi,
        ("", "a"),
        &[KeyPress::ESC, KeyPress::from('x'), KeyPress::ENTER],
        ("", ""),
    );
}

#[test]
fn uppercase_x() {
    assert_cursor(
        EditMode::Vi,
        ("Hi", ""),
        &[KeyPress::ESC, KeyPress::from('X'), KeyPress::ENTER],
        ("", "i"),
    );
}

#[test]
fn h() {
    for key in &[
        KeyPress::from('h'),
        KeyPress::ctrl('H'),
        KeyPress::BACKSPACE,
    ] {
        assert_cursor(
            EditMode::Vi,
            ("Bye", ""),
            &[KeyPress::ESC, *key, KeyPress::ENTER],
            ("B", "ye"),
        );
        assert_cursor(
            EditMode::Vi,
            ("Bye", ""),
            &[KeyPress::ESC, KeyPress::from('2'), *key, KeyPress::ENTER],
            ("", "Bye"),
        );
    }
}

#[test]
fn l() {
    for key in &[KeyPress::from('l'), KeyPress::from(' ')] {
        assert_cursor(
            EditMode::Vi,
            ("", "Hi"),
            &[KeyPress::ESC, *key, KeyPress::ENTER],
            ("H", "i"),
        );
        assert_cursor(
            EditMode::Vi,
            ("", "Hi"),
            &[KeyPress::ESC, KeyPress::from('2'), *key, KeyPress::ENTER],
            ("Hi", ""),
        );
    }
}

#[test]
fn j() {
    for key in &[KeyPress::from('j'), KeyPress::from('+')] {

        assert_cursor(
            EditMode::Vi,
            ("Hel", "lo,\nworld!"),
            // NOTE: escape moves backwards on char
            &[KeyPress::ESC, *key, KeyPress::ENTER],
            ("Hello,\nwo", "rld!"),
        );
        assert_cursor(
            EditMode::Vi,
            ("", "One\nTwo\nThree"),
            &[KeyPress::ESC, KeyPress::from('2'), *key, KeyPress::ENTER],
            ("One\nTwo\n", "Three"),
        );
        assert_cursor(
            EditMode::Vi,
            ("Hel", "lo,\nworld!"),
            // NOTE: escape moves backwards on char
            &[KeyPress::ESC, KeyPress::from('7'), *key, KeyPress::ENTER],
            ("Hello,\nwo", "rld!"),
        );
    }
}

#[test]
fn k() {
    for key in &[KeyPress::from('k'), KeyPress::from('-')] {
        assert_cursor(
            EditMode::Vi,
            ("Hello,\nworl", "d!"),
            // NOTE: escape moves backwards on char
            &[KeyPress::ESC, *key, KeyPress::ENTER],
            ("Hel", "lo,\nworld!"),
        );
        assert_cursor(
            EditMode::Vi,
            ("One\nTwo\nT", "hree"),
            // NOTE: escape moves backwards on char
            &[KeyPress::ESC, KeyPress::from('2'), *key, KeyPress::ENTER],
            ("", "One\nTwo\nThree"),
        );
        assert_cursor(
            EditMode::Vi,
            ("Hello,\nworl", "d!"),
            // NOTE: escape moves backwards on char
            &[KeyPress::ESC, KeyPress::from('5'), *key, KeyPress::ENTER],
            ("Hel", "lo,\nworld!"),
        );
        assert_cursor(
            EditMode::Vi,
            ("first line\nshort\nlong line", ""),
            &[KeyPress::ESC, *key, KeyPress::ENTER],
            ("first line\nshort", "\nlong line"),
        );
    }
}

#[test]
fn ctrl_n() {
    for key in &[KeyPress::ctrl('N')] {
        assert_history(
            EditMode::Vi,
            &["line1", "line2"],
            &[
                KeyPress::ESC,
                KeyPress::ctrl('P'),
                KeyPress::ctrl('P'),
                *key,
                KeyPress::ENTER,
            ],
            "",
            ("line2", ""),
        );
    }
}

#[test]
fn ctrl_p() {
    for key in &[KeyPress::ctrl('P')] {
        assert_history(
            EditMode::Vi,
            &["line1"],
            &[KeyPress::ESC, *key, KeyPress::ENTER],
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
            KeyPress::ESC,
            KeyPress::ctrl('W'),
            KeyPress::from('p'),
            KeyPress::ENTER,
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
            KeyPress::ESC,
            KeyPress::ctrl('W'),
            KeyPress::from('P'),
            KeyPress::ENTER,
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
            KeyPress::ESC,
            KeyPress::from('r'),
            KeyPress::from('o'),
            KeyPress::ENTER,
        ],
        ("H", "o, world!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("He", "llo, world!"),
        &[
            KeyPress::ESC,
            KeyPress::from('4'),
            KeyPress::from('r'),
            KeyPress::from('i'),
            KeyPress::ENTER,
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
            KeyPress::ESC,
            KeyPress::from('s'),
            KeyPress::from('o'),
            KeyPress::ENTER,
        ],
        ("Ho", ", world!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("He", "llo, world!"),
        &[
            KeyPress::ESC,
            KeyPress::from('4'),
            KeyPress::from('s'),
            KeyPress::from('i'),
            KeyPress::ENTER,
        ],
        ("Hi", ", world!"),
    );
}

#[test]
fn uppercase_s() {
    assert_cursor(
        EditMode::Vi,
        ("Hello, ", "world"),
        &[KeyPress::ESC, KeyPress::from('S'), KeyPress::ENTER],
        ("", ""),
    );
}

#[test]
fn t() {
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[
            KeyPress::ESC,
            KeyPress::from('t'),
            KeyPress::from('r'),
            KeyPress::ENTER,
        ],
        ("Hello, w", "orld!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[
            KeyPress::ESC,
            KeyPress::from('3'),
            KeyPress::from('t'),
            KeyPress::from('l'),
            KeyPress::ENTER,
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
            KeyPress::ESC,
            KeyPress::from('T'),
            KeyPress::from('r'),
            KeyPress::ENTER,
        ],
        ("Hello, wor", "ld!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("Hello, world!", ""),
        &[
            KeyPress::ESC,
            KeyPress::from('3'),
            KeyPress::from('T'),
            KeyPress::from('l'),
            KeyPress::ENTER,
        ],
        ("Hel", "lo, world!"),
    );
}
