pub fn fib(n: u32) -> u32 {
    let (mut a, mut b) = (1, 1);
    for _ in 2..=n {
        (a, b) = (b, a + b);
    }
    a
}

#[test]
fn test_fib() {
    assert_eq!(55, fib(10));
}
