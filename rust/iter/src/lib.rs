pub fn position(l: &[i32], v: i32) -> Option<usize> {
    let mut i = 0;
    for e in l {
        if *e == v {
            break;
        }
        i += 1;
    }
    if i == l.len() {
        None
    } else {
        Some(i)
    }
}

#[test]
fn test_position() {
    let l = [1, 2, 3, 4, 5];
    assert_eq!(Some(2), position(&l, 3));
    assert_eq!(None, position(&l, 6));
}
