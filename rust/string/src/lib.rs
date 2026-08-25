pub fn char_at(pattern: &str, i: usize) -> char {
    pattern[i..].chars().next().unwrap()
}

pub fn is_eof(pattern: &str, offset: usize) -> bool {
    offset == pattern.len()
}

pub fn bump(pattern: &str, offset: usize) -> usize {
    offset + char_at(pattern, offset).len_utf8()
}

pub fn count_chars(pattern: &str) -> usize {
    let mut offset = 0;
    let mut count = 0;
    while !is_eof(pattern, offset) {
        offset = bump(pattern, offset);
        count += 1;
    }
    count
}

pub fn count_lines(pattern: &str) -> usize {
    let mut offset = 0;
    let mut line = 1;
    while !is_eof(pattern, offset) {
        if char_at(pattern, offset) == '\n' {
            line += 1;
        }
        offset = bump(pattern, offset);
    }
    line
}

const PATTERN: &str = "a√\nz";

#[test]
fn test_char_at() {
    assert_eq!('a', char_at(PATTERN, 0));
    assert_eq!('√', char_at(PATTERN, 1));
    assert_eq!('\n', char_at(PATTERN, 4));
    assert_eq!('z', char_at(PATTERN, 5));
}

#[test]
fn test_is_eof() {
    assert!(!is_eof(PATTERN, 0));
    assert!(is_eof(PATTERN, 6));
}

#[test]
fn test_len_utf8() {
    assert_eq!(1, char_at(PATTERN, 0).len_utf8());
    assert_eq!(3, char_at(PATTERN, 1).len_utf8());
}

#[test]
fn test_count_chars() {
    assert_eq!(4, count_chars(PATTERN));
}

#[test]
fn test_count_lines() {
    assert_eq!(2, count_lines(PATTERN));
}
