enum Result<T, E> {
    Ok(T),
    Err(E),
}

impl<T, E> Result<T, E> {
    fn is_ok(&self) -> bool {
        match self {
            Result::Ok(_) => true,
            Result::Err(_) => false,
        }
    }

    fn is_err(&self) -> bool {
        match self {
            Result::Ok(_) => false,
            Result::Err(_) => true,
        }
    }

}

fn main() {
    let x: Result<u32, u32> = Result::Ok(42);
    let y: Result<u32, u32> = Result::Err(1);
    println!("{}", x.is_ok());
    println!("{}", y.is_err());
}
