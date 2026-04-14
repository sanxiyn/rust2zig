enum Option<T> {
    Some(T),
    None,
}

impl<T> Option<T> {
    fn is_some(&self) -> bool {
        match self {
            Option::Some(_) => true,
            Option::None => false,
        }
    }

    fn unwrap(self) -> T {
        match self {
            Option::Some(x) => x,
            Option::None => panic!("called unwrap on None"),
        }
    }
}

fn main() {
    let x: Option<u32> = Option::Some(42);
    let y: Option<u32> = Option::None;
    println!("{}", x.is_some());
    println!("{}", y.is_some());
    println!("{}", x.unwrap());
}
