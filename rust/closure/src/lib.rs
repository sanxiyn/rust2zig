#[test]
fn test_closure() {
    let x = 3;
    let double = |x| x * 2;
    assert_eq!(6, double(x));
}

#[test]
fn test_capture() {
    let a = 3;
    let add = |x| x + a;
    assert_eq!(6, add(3));
}
