pub fn sum(xs: &[i32]) -> i32 {
    let mut total = 0;
    for x in xs {
        total += *x;
    }
    total
}

#[test]
fn test_sum() {
    let xs = [1, 2, 3, 4, 5];
    let mut total = 0;
    for x in xs {
        total += x;
    }
    assert_eq!(15, total);
    assert_eq!(15, sum(&xs));
    total = 0;
    for x in 1..=5 {
        total += x;
    }
    assert_eq!(15, total);
}
