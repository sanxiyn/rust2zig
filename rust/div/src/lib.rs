pub fn div(a: u32, b: u32) -> Option<u32> {
    if a % b == 0 {
        Some(a / b)
    } else {
        None
    }
}

pub fn div2(a: u32, b: u32) -> u32 {
    if let Some(x) = div(a, b) {
        x
    } else {
        0
    }
}

#[test]
fn test_div() {
    assert_eq!(Some(2), div(6, 3));
    assert_eq!(None, div(7, 3));
}

#[test]
fn test_div2() {
    assert_eq!(2, div2(6, 3));
    assert_eq!(0, div2(7, 3));
}
