pub enum Nat {
    Zero,
    Succ(Box<Nat>),
}

impl Nat {
    pub fn succ(n: Nat) -> Nat {
        Nat::Succ(Box::new(n))
    }

    pub fn to_int(&self) -> i32 {
        match self {
            Nat::Zero => 0,
            Nat::Succ(n) => 1 + n.to_int(),
        }
    }
}

#[test]
fn test_to_int() {
    let three = Nat::succ(Nat::succ(Nat::succ(Nat::Zero)));
    assert_eq!(3, three.to_int());
}
