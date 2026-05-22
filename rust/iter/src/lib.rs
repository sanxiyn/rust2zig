pub fn position<T: PartialEq>(l: &[T], v: T) -> Option<usize> {
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
    let l: &[i32] = &[1, 2, 3, 4, 5];
    let v = 3;
    assert_eq!(Some(2), position(l, v));
    let v = 6;
    assert_eq!(None, position(l, v));
}
