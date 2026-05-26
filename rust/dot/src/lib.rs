pub fn dot(a: &[i32], b: &[i32]) -> i32 {
    let mut sum = 0;
    for (x, y) in std::iter::zip(a, b) {
        sum += x * y;
    }
    sum
}

#[test]
fn test_dot() {
    let a = [1, 2, 3];
    let b = [4, 5, 6];
    assert_eq!(32, dot(&a, &b));
}
