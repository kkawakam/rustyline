///! Basic commands tests.
use super::{assert_cursor, assert_line, assert_line_with_initial, init_editor};
use config::EditMode;
use consts::KeyPress;
use error::ReadlineError;

#[test]
fn home_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_cursor(
            *mode,
            ("", ""),
            &[KeyPress::Home, KeyPress::Enter],
            ("", ""),
        );
        assert_cursor(
            *mode,
            ("Hi", ""),
            &[KeyPress::Home, KeyPress::Enter],
            ("", "Hi"),
        );
    }
}

#[test]
fn end_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_cursor(*mode, ("", ""), &[KeyPress::End, KeyPress::Enter], ("", ""));
        assert_cursor(
            *mode,
            ("H", "i"),
            &[KeyPress::End, KeyPress::Enter],
            ("Hi", ""),
        );
        assert_cursor(
            *mode,
            ("", "Hi"),
            &[KeyPress::End, KeyPress::Enter],
            ("Hi", ""),
        );
    }
}

#[test]
fn left_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_cursor(
            *mode,
            ("Hi", ""),
            &[KeyPress::Left, KeyPress::Enter],
            ("H", "i"),
        );
        assert_cursor(
            *mode,
            ("H", "i"),
            &[KeyPress::Left, KeyPress::Enter],
            ("", "Hi"),
        );
        assert_cursor(
            *mode,
            ("", "Hi"),
            &[KeyPress::Left, KeyPress::Enter],
            ("", "Hi"),
        );
    }
}

#[test]
fn right_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_cursor(
            *mode,
            ("", ""),
            &[KeyPress::Right, KeyPress::Enter],
            ("", ""),
        );
        assert_cursor(
            *mode,
            ("", "Hi"),
            &[KeyPress::Right, KeyPress::Enter],
            ("H", "i"),
        );
        assert_cursor(
            *mode,
            ("B", "ye"),
            &[KeyPress::Right, KeyPress::Enter],
            ("By", "e"),
        );
        assert_cursor(
            *mode,
            ("H", "i"),
            &[KeyPress::Right, KeyPress::Enter],
            ("Hi", ""),
        );
    }
}

#[test]
fn enter_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_line(*mode, &[KeyPress::Enter], "");
        assert_line(*mode, &[KeyPress::Char('a'), KeyPress::Enter], "a");
        assert_line_with_initial(*mode, ("Hi", ""), &[KeyPress::Enter], "Hi");
        assert_line_with_initial(*mode, ("", "Hi"), &[KeyPress::Enter], "Hi");
        assert_line_with_initial(*mode, ("H", "i"), &[KeyPress::Enter], "Hi");
    }
}

#[test]
fn newline_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_line(*mode, &[KeyPress::Ctrl('J')], "");
        assert_line(*mode, &[KeyPress::Char('a'), KeyPress::Ctrl('J')], "a");
    }
}

#[test]
fn eof_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        let mut editor = init_editor(*mode, &[KeyPress::Ctrl('D')]);
        let err = editor.readline(">>");
        assert_matches!(err, Err(ReadlineError::Eof));
    }
    assert_line(
        EditMode::Emacs,
        &[KeyPress::Char('a'), KeyPress::Ctrl('D'), KeyPress::Enter],
        "a",
    );
    assert_line(
        EditMode::Vi,
        &[KeyPress::Char('a'), KeyPress::Ctrl('D')],
        "a",
    );
    assert_line_with_initial(
        EditMode::Emacs,
        ("", "Hi"),
        &[KeyPress::Ctrl('D'), KeyPress::Enter],
        "i",
    );
    assert_line_with_initial(EditMode::Vi, ("", "Hi"), &[KeyPress::Ctrl('D')], "Hi");
}

#[test]
fn interrupt_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        let mut editor = init_editor(*mode, &[KeyPress::Ctrl('C')]);
        let err = editor.readline(">>");
        assert_matches!(err, Err(ReadlineError::Interrupted));

        let mut editor = init_editor(*mode, &[KeyPress::Ctrl('C')]);
        let err = editor.readline_with_initial(">>", ("Hi", ""));
        assert_matches!(err, Err(ReadlineError::Interrupted));
    }
}

#[test]
fn delete_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_cursor(
            *mode,
            ("a", ""),
            &[KeyPress::Delete, KeyPress::Enter],
            ("a", ""),
        );
        assert_cursor(
            *mode,
            ("", "a"),
            &[KeyPress::Delete, KeyPress::Enter],
            ("", ""),
        );
    }
}

#[test]
fn ctrl_t() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_cursor(
            *mode,
            ("a", "b"),
            &[KeyPress::Ctrl('T'), KeyPress::Enter],
            ("ba", ""),
        );
        assert_cursor(
            *mode,
            ("ab", "cd"),
            &[KeyPress::Ctrl('T'), KeyPress::Enter],
            ("acb", "d"),
        );
    }
}

#[test]
fn ctrl_u() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_cursor(
            *mode,
            ("start of line ", "end"),
            &[KeyPress::Ctrl('U'), KeyPress::Enter],
            ("", "end"),
        );
        assert_cursor(
            *mode,
            ("", "end"),
            &[KeyPress::Ctrl('U'), KeyPress::Enter],
            ("", "end"),
        );
    }
}

#[test]
fn ctrl_v() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_cursor(
            *mode,
            ("", ""),
            &[KeyPress::Ctrl('V'), KeyPress::Char('\t'), KeyPress::Enter],
            ("\t", ""),
        );
    }
}

#[test]
fn ctrl_w() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_cursor(
            *mode,
            ("Hello, ", "world"),
            &[KeyPress::Ctrl('W'), KeyPress::Enter],
            ("", "world"),
        );
        assert_cursor(
            *mode,
            ("Hello, world.", ""),
            &[KeyPress::Ctrl('W'), KeyPress::Enter],
            ("Hello, ", ""),
        );
    }
}

#[test]
fn ctrl_y() {
    for mode in &[EditMode::Emacs /* FIXME, EditMode::Vi */] {
        assert_cursor(
            *mode,
            ("Hello, ", "world"),
            &[KeyPress::Ctrl('W'), KeyPress::Ctrl('Y'), KeyPress::Enter],
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
            &[KeyPress::Ctrl('W'), KeyPress::Ctrl('_'), KeyPress::Enter],
            ("Hello, ", "world"),
        );
    }
}
