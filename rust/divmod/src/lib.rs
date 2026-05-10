pub fn divmod(a: u32, b: u32) -> (u32, u32) {
    (a / b, a % b)
}

#[test]
fn test_divmod() {
    let (q, r) = divmod(7, 3);
    assert_eq!(2, q);
    assert_eq!(1, r);
}
