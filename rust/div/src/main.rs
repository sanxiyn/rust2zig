fn div(a: u32, b: u32) -> Option<u32> {
    if a % b == 0 {
        Some(a / b)
    } else {
        None
    }
}

fn main() {
    if let Some(x) = div(6, 3) {
        println!("{}", x);
    } else {
        println!("not divisible");
    }
    if let Some(x) = div(7, 3) {
        println!("{}", x);
    } else {
        println!("not divisible");
    }
}
