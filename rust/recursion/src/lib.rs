pub fn fib(n: u32) -> u32 {
    if n < 2 {
        return n;
    }
    fib(n - 1) + fib(n - 2)
}

pub fn is_even(n: u32) -> bool {
    if n == 0 {
        return true;
    }
    is_odd(n - 1)
}

pub fn is_odd(n: u32) -> bool {
    if n == 0 {
        return false;
    }
    is_even(n - 1)
}

#[test]
fn test_fib() {
    assert_eq!(55, fib(10));
}

#[test]
fn test_parity() {
    assert_eq!(true, is_even(10));
    assert_eq!(false, is_odd(10));
}
