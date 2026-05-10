pub enum Option<T> {
    Some(T),
    None,
}

impl<T> Option<T> {
    pub fn and<U>(self, optb: Option<U>) -> Option<U> {
        match self {
            Option::Some(_) => optb,
            Option::None => Option::None,
        }
    }

    pub fn is_none(&self) -> bool {
        match self {
            Option::Some(_) => false,
            Option::None => true,
        }
    }

    pub fn is_some(&self) -> bool {
        match self {
            Option::Some(_) => true,
            Option::None => false,
        }
    }

    pub fn unwrap(self) -> T {
        match self {
            Option::Some(x) => x,
            Option::None => panic!("called unwrap on None"),
        }
    }
}

#[test]
fn test_option() {
    let x: Option<u32> = Option::Some(42);
    let y: Option<u32> = Option::None;
    assert_eq!(true, x.is_some());
    assert_eq!(false, y.is_some());
    assert_eq!(42, x.unwrap());
    let z: Option<i32> = Option::Some(7);
    assert_eq!(true, y.and(z).is_none());
}
