fn main() {
    let xs: [u32; _] = [1, 2, 3, 4, 5];
    let mut total: u32 = 0;
    for x in xs {
        total += x;
    }
    println!("{}", total);
}
