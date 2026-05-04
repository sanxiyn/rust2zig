fn divmod(a: u32, b: u32) -> (u32, u32) {
    (a / b, a % b)
}

fn main() {
    let (q, r) = divmod(7, 3);
    println!("{} {}", q, r);
}
