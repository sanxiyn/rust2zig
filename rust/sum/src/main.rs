fn sum(xs: &[i32]) -> i32 {
    let mut total = 0;
    for x in xs {
        total += x;
    }
    total
}

fn main() {
    let xs = [1, 2, 3, 4, 5];
    let mut total = 0;
    for x in xs {
        total += x;
    }
    println!("{}", total);
    println!("{}", sum(&xs));
    total = 0;
    for x in 1..=5 {
        total += x;
    }
    println!("{}", total);
}
