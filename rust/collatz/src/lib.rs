pub fn collatz(start: u32) -> Vec<u32> {
    let mut steps = vec![];
    let mut n = start;
    loop {
        steps.push(n);
        if n == 1 {
            break;
        }
        if n % 2 == 0 {
            n = n / 2;
        } else {
            n = 3 * n + 1;
        }
    }
    steps
}

#[test]
fn test_collatz() {
    let mut steps = collatz(3);
    assert_eq!(8, steps.len());
    assert_eq!(3, steps[0]);
    assert_eq!(10, steps[1]);
    assert_eq!(1, steps[7]);
    assert_eq!(Some(1), steps.pop());
    assert_eq!(7, steps.len());
}
