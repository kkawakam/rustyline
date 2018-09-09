//! History related commands tests
use super::assert_history;
use config::EditMode;
use keys::KeyPress;

#[test]
fn down_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_history(
            *mode,
            &["line1"],
            &[KeyPress::Down, KeyPress::Enter],
            ("", ""),
        );
        assert_history(
            *mode,
            &["line1", "line2"],
            &[KeyPress::Up, KeyPress::Up, KeyPress::Down, KeyPress::Enter],
            ("line2", ""),
        );
        assert_history(
            *mode,
            &["line1"],
            &[
                KeyPress::Char('a'),
                KeyPress::Up,
                KeyPress::Down, // restore original line
                KeyPress::Enter,
            ],
            ("a", ""),
        );
        assert_history(
            *mode,
            &["line1"],
            &[
                KeyPress::Char('a'),
                KeyPress::Down, // noop
                KeyPress::Enter,
            ],
            ("a", ""),
        );
    }
}

#[test]
fn up_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_history(*mode, &[], &[KeyPress::Up, KeyPress::Enter], ("", ""));
        assert_history(
            *mode,
            &["line1"],
            &[KeyPress::Up, KeyPress::Enter],
            ("line1", ""),
        );
        assert_history(
            *mode,
            &["line1", "line2"],
            &[KeyPress::Up, KeyPress::Up, KeyPress::Enter],
            ("line1", ""),
        );
    }
}

#[test]
fn ctrl_r() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_history(
            *mode,
            &[],
            &[KeyPress::Ctrl('R'), KeyPress::Char('o'), KeyPress::Enter],
            ("o", ""),
        );
        assert_history(
            *mode,
            &["rustc", "cargo"],
            &[
                KeyPress::Ctrl('R'),
                KeyPress::Char('o'),
                KeyPress::Right, // just to assert cursor pos
                KeyPress::Enter,
            ],
            ("cargo", ""),
        );
        assert_history(
            *mode,
            &["rustc", "cargo"],
            &[
                KeyPress::Ctrl('R'),
                KeyPress::Char('u'),
                KeyPress::Right, // just to assert cursor pos
                KeyPress::Enter,
            ],
            ("ru", "stc"),
        );
        assert_history(
            *mode,
            &["rustc", "cargo"],
            &[
                KeyPress::Ctrl('R'),
                KeyPress::Char('r'),
                KeyPress::Char('u'),
                KeyPress::Right, // just to assert cursor pos
                KeyPress::Enter,
            ],
            ("r", "ustc"),
        );
        assert_history(
            *mode,
            &["rustc", "cargo"],
            &[
                KeyPress::Ctrl('R'),
                KeyPress::Char('r'),
                KeyPress::Ctrl('R'),
                KeyPress::Right, // just to assert cursor pos
                KeyPress::Enter,
            ],
            ("r", "ustc"),
        );
        assert_history(
            *mode,
            &["rustc", "cargo"],
            &[
                KeyPress::Ctrl('R'),
                KeyPress::Char('r'),
                KeyPress::Char('z'), // no match
                KeyPress::Right,     // just to assert cursor pos
                KeyPress::Enter,
            ],
            ("car", "go"),
        );
        assert_history(
            EditMode::Emacs,
            &["rustc", "cargo"],
            &[
                KeyPress::Char('a'),
                KeyPress::Ctrl('R'),
                KeyPress::Char('r'),
                KeyPress::Ctrl('G'), // abort (FIXME: doesn't work with vi mode)
                KeyPress::Enter,
            ],
            ("a", ""),
        );
    }
}

#[test]
fn ctrl_s() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_history(
            *mode,
            &["rustc", "cargo"],
            &[
                KeyPress::Ctrl('R'),
                KeyPress::Char('r'),
                KeyPress::Ctrl('R'),
                KeyPress::Ctrl('S'),
                KeyPress::Right, // just to assert cursor pos
                KeyPress::Enter,
            ],
            ("car", "go"),
        );
    }
}

#[test]
fn meta_lt() {
    assert_history(
        EditMode::Emacs,
        &[""],
        &[KeyPress::Meta('<'), KeyPress::Enter],
        ("", ""),
    );
    assert_history(
        EditMode::Emacs,
        &["rustc", "cargo"],
        &[KeyPress::Meta('<'), KeyPress::Enter],
        ("rustc", ""),
    );
}

#[test]
fn meta_gt() {
    assert_history(
        EditMode::Emacs,
        &[""],
        &[KeyPress::Meta('>'), KeyPress::Enter],
        ("", ""),
    );
    assert_history(
        EditMode::Emacs,
        &["rustc", "cargo"],
        &[KeyPress::Meta('<'), KeyPress::Meta('>'), KeyPress::Enter],
        ("", ""),
    );
    assert_history(
        EditMode::Emacs,
        &["rustc", "cargo"],
        &[
            KeyPress::Char('a'),
            KeyPress::Meta('<'),
            KeyPress::Meta('>'), // restore original line
            KeyPress::Enter,
        ],
        ("a", ""),
    );
}
