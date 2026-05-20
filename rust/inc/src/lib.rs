pub fn inc(x: &mut i32) {
    *x += 1;
}

pub fn succ(x: &i32) -> i32 {
    *x + 1
}

#[test]
fn test_inc() {
    let mut x = 2;
    let y = x;
    inc(&mut x);
    assert_eq!(succ(&y), x);
}
