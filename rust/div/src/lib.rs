pub fn div(a: u32, b: u32) -> Option<u32> {
    if a % b == 0 {
        Some(a / b)
    } else {
        None
    }
}

#[test]
fn test_div() {
    assert_eq!(Some(2), div(6, 3));
    assert_eq!(None, div(7, 3));
}
