///! Basic commands tests.
use super::{assert_cursor, assert_line, assert_line_with_initial, init_editor};
use crate::config::EditMode;
use crate::error::ReadlineError;
use crate::keys::{Key, KeyPress};

#[test]
fn home_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_cursor(
            *mode,
            ("", ""),
            &[Key::Home.into(), Key::Enter.into()],
            ("", ""),
        );
        assert_cursor(
            *mode,
            ("Hi", ""),
            &[Key::Home.into(), Key::Enter.into()],
            ("", "Hi"),
        );
        if *mode == EditMode::Vi {
            // vi command mode
            assert_cursor(
                *mode,
                ("Hi", ""),
                &[Key::Esc.into(), Key::Home.into(), Key::Enter.into()],
                ("", "Hi"),
            );
        }
    }
}

#[test]
fn end_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_cursor(*mode, ("", ""), &[KeyPress::END, KeyPress::ENTER], ("", ""));
        assert_cursor(
            *mode,
            ("H", "i"),
            &[KeyPress::END, KeyPress::ENTER],
            ("Hi", ""),
        );
        assert_cursor(
            *mode,
            ("", "Hi"),
            &[KeyPress::END, KeyPress::ENTER],
            ("Hi", ""),
        );
        if *mode == EditMode::Vi {
            // vi command mode
            assert_cursor(
                *mode,
                ("", "Hi"),
                &[KeyPress::ESC, KeyPress::END, KeyPress::ENTER],
                ("Hi", ""),
            );
        }
    }
}

#[test]
fn left_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_cursor(
            *mode,
            ("Hi", ""),
            &[KeyPress::LEFT, KeyPress::ENTER],
            ("H", "i"),
        );
        assert_cursor(
            *mode,
            ("H", "i"),
            &[KeyPress::LEFT, KeyPress::ENTER],
            ("", "Hi"),
        );
        assert_cursor(
            *mode,
            ("", "Hi"),
            &[KeyPress::LEFT, KeyPress::ENTER],
            ("", "Hi"),
        );
        if *mode == EditMode::Vi {
            // vi command mode
            assert_cursor(
                *mode,
                ("Bye", ""),
                &[KeyPress::ESC, KeyPress::LEFT, KeyPress::ENTER],
                ("B", "ye"),
            );
        }
    }
}

#[test]
fn right_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_cursor(
            *mode,
            ("", ""),
            &[KeyPress::RIGHT, KeyPress::ENTER],
            ("", ""),
        );
        assert_cursor(
            *mode,
            ("", "Hi"),
            &[KeyPress::RIGHT, KeyPress::ENTER],
            ("H", "i"),
        );
        assert_cursor(
            *mode,
            ("B", "ye"),
            &[KeyPress::RIGHT, KeyPress::ENTER],
            ("By", "e"),
        );
        assert_cursor(
            *mode,
            ("H", "i"),
            &[KeyPress::RIGHT, KeyPress::ENTER],
            ("Hi", ""),
        );
        if *mode == EditMode::Vi {
            // vi command mode
            assert_cursor(
                *mode,
                ("", "Hi"),
                &[KeyPress::ESC, KeyPress::RIGHT, KeyPress::ENTER],
                ("H", "i"),
            );
        }
    }
}

#[test]
fn enter_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_line(*mode, &[KeyPress::ENTER], "");
        assert_line(*mode, &[KeyPress::normal('a'), KeyPress::ENTER], "a");
        assert_line_with_initial(*mode, ("Hi", ""), &[KeyPress::ENTER], "Hi");
        assert_line_with_initial(*mode, ("", "Hi"), &[KeyPress::ENTER], "Hi");
        assert_line_with_initial(*mode, ("H", "i"), &[KeyPress::ENTER], "Hi");
        if *mode == EditMode::Vi {
            // vi command mode
            assert_line(*mode, &[KeyPress::ESC, KeyPress::ENTER], "");
            assert_line(
                *mode,
                &[KeyPress::normal('a'), KeyPress::ESC, KeyPress::ENTER],
                "a",
            );
            assert_line_with_initial(*mode, ("Hi", ""), &[KeyPress::ESC, KeyPress::ENTER], "Hi");
            assert_line_with_initial(*mode, ("", "Hi"), &[KeyPress::ESC, KeyPress::ENTER], "Hi");
            assert_line_with_initial(*mode, ("H", "i"), &[KeyPress::ESC, KeyPress::ENTER], "Hi");
        }
    }
}

#[test]
fn newline_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_line(*mode, &[KeyPress::ctrl('J')], "");
        assert_line(*mode, &[KeyPress::normal('a'), KeyPress::ctrl('J')], "a");
        if *mode == EditMode::Vi {
            // vi command mode
            assert_line(*mode, &[KeyPress::ESC, KeyPress::ctrl('J')], "");
            assert_line(
                *mode,
                &[KeyPress::normal('a'), KeyPress::ESC, KeyPress::ctrl('J')],
                "a",
            );
        }
    }
}

#[test]
fn eof_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        let mut editor = init_editor(*mode, &[KeyPress::ctrl('D')]);
        let err = editor.readline(">>");
        assert_matches!(err, Err(ReadlineError::Eof));
    }
    assert_line(
        EditMode::Emacs,
        &[KeyPress::normal('a'), KeyPress::ctrl('D'), KeyPress::ENTER],
        "a",
    );
    assert_line(
        EditMode::Vi,
        &[KeyPress::normal('a'), KeyPress::ctrl('D')],
        "a",
    );
    assert_line(
        EditMode::Vi,
        &[KeyPress::normal('a'), KeyPress::ESC, KeyPress::ctrl('D')],
        "a",
    );
    assert_line_with_initial(
        EditMode::Emacs,
        ("", "Hi"),
        &[KeyPress::ctrl('D'), KeyPress::ENTER],
        "i",
    );
    assert_line_with_initial(EditMode::Vi, ("", "Hi"), &[KeyPress::ctrl('D')], "Hi");
    assert_line_with_initial(
        EditMode::Vi,
        ("", "Hi"),
        &[KeyPress::ESC, KeyPress::ctrl('D')],
        "Hi",
    );
}

#[test]
fn interrupt_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        let mut editor = init_editor(*mode, &[KeyPress::ctrl('C')]);
        let err = editor.readline(">>");
        assert_matches!(err, Err(ReadlineError::Interrupted));

        let mut editor = init_editor(*mode, &[KeyPress::ctrl('C')]);
        let err = editor.readline_with_initial(">>", ("Hi", ""));
        assert_matches!(err, Err(ReadlineError::Interrupted));
        if *mode == EditMode::Vi {
            // vi command mode
            let mut editor = init_editor(*mode, &[KeyPress::ESC, KeyPress::ctrl('C')]);
            let err = editor.readline_with_initial(">>", ("Hi", ""));
            assert_matches!(err, Err(ReadlineError::Interrupted));
        }
    }
}

#[test]
fn delete_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_cursor(
            *mode,
            ("a", ""),
            &[KeyPress::DELETE, KeyPress::ENTER],
            ("a", ""),
        );
        assert_cursor(
            *mode,
            ("", "a"),
            &[KeyPress::DELETE, KeyPress::ENTER],
            ("", ""),
        );
        if *mode == EditMode::Vi {
            // vi command mode
            assert_cursor(
                *mode,
                ("", "a"),
                &[KeyPress::ESC, KeyPress::DELETE, KeyPress::ENTER],
                ("", ""),
            );
        }
    }
}

#[test]
fn ctrl_t() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_cursor(
            *mode,
            ("a", "b"),
            &[KeyPress::ctrl('T'), KeyPress::ENTER],
            ("ba", ""),
        );
        assert_cursor(
            *mode,
            ("ab", "cd"),
            &[KeyPress::ctrl('T'), KeyPress::ENTER],
            ("acb", "d"),
        );
        if *mode == EditMode::Vi {
            // vi command mode
            assert_cursor(
                *mode,
                ("ab", ""),
                &[KeyPress::ESC, KeyPress::ctrl('T'), KeyPress::ENTER],
                ("ba", ""),
            );
        }
    }
}

#[test]
fn ctrl_u() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_cursor(
            *mode,
            ("start of line ", "end"),
            &[KeyPress::ctrl('U'), KeyPress::ENTER],
            ("", "end"),
        );
        assert_cursor(
            *mode,
            ("", "end"),
            &[KeyPress::ctrl('U'), KeyPress::ENTER],
            ("", "end"),
        );
        if *mode == EditMode::Vi {
            // vi command mode
            assert_cursor(
                *mode,
                ("start of line ", "end"),
                &[KeyPress::ESC, KeyPress::ctrl('U'), KeyPress::ENTER],
                ("", " end"),
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn ctrl_v() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_cursor(
            *mode,
            ("", ""),
            &[KeyPress::ctrl('V'), KeyPress::normal('\t'), KeyPress::ENTER],
            ("\t", ""),
        );
        if *mode == EditMode::Vi {
            // vi command mode
            assert_cursor(
                *mode,
                ("", ""),
                &[
                    KeyPress::ESC,
                    KeyPress::ctrl('V'),
                    KeyPress::normal('\t'),
                    KeyPress::ENTER,
                ],
                ("\t", ""),
            );
        }
    }
}

#[test]
fn ctrl_w() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_cursor(
            *mode,
            ("Hello, ", "world"),
            &[KeyPress::ctrl('W'), KeyPress::ENTER],
            ("", "world"),
        );
        assert_cursor(
            *mode,
            ("Hello, world.", ""),
            &[KeyPress::ctrl('W'), KeyPress::ENTER],
            ("Hello, ", ""),
        );
        if *mode == EditMode::Vi {
            // vi command mode
            assert_cursor(
                *mode,
                ("Hello, world.", ""),
                &[KeyPress::ESC, KeyPress::ctrl('W'), KeyPress::ENTER],
                ("Hello, ", "."),
            );
        }
    }
}

#[test]
fn ctrl_y() {
    for mode in &[EditMode::Emacs /* FIXME, EditMode::Vi */] {
        assert_cursor(
            *mode,
            ("Hello, ", "world"),
            &[KeyPress::ctrl('W'), KeyPress::ctrl('Y'), KeyPress::ENTER],
            ("Hello, ", "world"),
        );
    }
}

#[test]
fn ctrl__() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_cursor(
            *mode,
            ("Hello, ", "world"),
            &[KeyPress::ctrl('W'), KeyPress::ctrl('_'), KeyPress::ENTER],
            ("Hello, ", "world"),
        );
        if *mode == EditMode::Vi {
            // vi command mode
            assert_cursor(
                *mode,
                ("Hello, ", "world"),
                &[
                    KeyPress::ESC,
                    KeyPress::ctrl('W'),
                    KeyPress::ctrl('_'),
                    KeyPress::ENTER,
                ],
                ("Hello,", " world"),
            );
        }
    }
}
