pub fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

#[derive(Clone, Copy)]
pub struct Ratio {
    pub num: u32,
    pub denom: u32,
}

impl Ratio {
    pub fn add(self, other: Ratio) -> Ratio {
        let n = self.num * other.denom + other.num * self.denom;
        let d = self.denom * other.denom;
        let g = gcd(n, d);
        Ratio { num: n / g, denom: d / g }
    }
}

#[test]
fn test_ratio() {
    let a = Ratio { num: 1, denom: 2 };
    let b = Ratio { num: 1, denom: 3 };
    let c = a.add(b);
    assert_eq!(5, c.num);
    assert_eq!(6, c.denom);
    let d = b.add(b);
    assert_eq!(2, d.num);
    assert_eq!(3, d.denom);
}
