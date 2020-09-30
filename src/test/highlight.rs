use crate::highlight::split_highlight;

#[test]
fn split_bold() {
    let (a, b) = split_highlight("\x1b[1mword1 word2\x1b[0m", 9);
    assert_eq!(a, "\x1b[1mword1\x1b[0m");
    assert_eq!(b, "\x1b[1m word2\x1b[0m");
}

#[test]
fn split_at_the_reset() {
    let (a, b) = split_highlight("\x1b[1mword1\x1b[0m word2", 9);
    assert_eq!(a, "\x1b[1mword1\x1b[0m");
    assert_eq!(b, " word2");
}

#[test]
fn split_nowhere() {
    let (a, b) = split_highlight("\x1b[1mword1\x1b[0m word2", 14);
    assert_eq!(a, "\x1b[1mword1\x1b[0m ");
    assert_eq!(b, "word2");
}
