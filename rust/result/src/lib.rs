pub enum Result<T, E> {
    Ok(T),
    Err(E),
}

impl<T, E> Result<T, E> {
    pub fn is_ok(&self) -> bool {
        match self {
            Result::Ok(_) => true,
            Result::Err(_) => false,
        }
    }

    pub fn is_err(&self) -> bool {
        match self {
            Result::Ok(_) => false,
            Result::Err(_) => true,
        }
    }

}

#[test]
fn test_result() {
    let x: Result<u32, u32> = Result::Ok(42);
    let y: Result<u32, u32> = Result::Err(1);
    assert_eq!(true, x.is_ok());
    assert_eq!(true, y.is_err());
}
