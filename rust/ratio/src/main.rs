fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

#[derive(Clone, Copy)]
struct Ratio {
    num: u32,
    denom: u32,
}

impl Ratio {
    fn add(self, other: Ratio) -> Ratio {
        let n = self.num * other.denom + other.num * self.denom;
        let d = self.denom * other.denom;
        let g = gcd(n, d);
        Ratio { num: n / g, denom: d / g }
    }
}

fn main() {
    let a = Ratio { num: 1, denom: 2 };
    let b = Ratio { num: 1, denom: 3 };
    let c = a.add(b);
    println!("{}/{}", c.num, c.denom);
    let d = b.add(b);
    println!("{}/{}", d.num, d.denom);
}
