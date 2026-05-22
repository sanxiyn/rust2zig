pub fn sum(xs: &[i32]) -> i32 {
    let mut total = 0;
    for x in xs {
        total += x;
    }
    total
}

pub fn sum_odd(xs: &[i32]) -> i32 {
    let mut total = 0;
    for x in xs {
        if x % 2 == 0 {
            continue;
        }
        total += x;
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

#[test]
fn test_sum_odd() {
    let xs = [1, 2, 3, 4, 5];
    assert_eq!(9, sum_odd(&xs));
}
