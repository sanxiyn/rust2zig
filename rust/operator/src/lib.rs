pub fn not_bool(b: bool) -> bool {
    !b
}

pub fn not_int(x: u8) -> u8 {
    !x
}

#[test]
fn test_not() {
    assert_eq!(false, not_bool(true));
    assert_eq!(true, not_bool(false));
    assert_eq!(0xf0, not_int(0x0f));
    assert_eq!(255, not_int(0));
}
