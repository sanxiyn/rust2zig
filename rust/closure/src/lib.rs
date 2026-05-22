#[test]
fn test_closure() {
    let x = 3;
    let double = |x| x * 2;
    assert_eq!(6, double(x));
}
