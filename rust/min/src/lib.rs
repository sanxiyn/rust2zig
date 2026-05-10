pub fn min<T: Ord>(a: T, b: T) -> T {
    if a < b { a } else { b }
}

#[test]
fn test_min() {
    let a = 2;
    let b = 3;
    assert_eq!(2, min(a, b));
    assert_eq!(2, min(b, a));
}
