#[derive(Debug, PartialEq)]
pub enum Error {
    Overflow,
    DivideByZero,
}

type Result<T> = core::result::Result<T, Error>;

const LIMIT: u32 = 1000;

pub fn add(a: u32, b: u32) -> Result<u32> {
    let sum = a + b;
    if sum > LIMIT {
        return Err(Error::Overflow);
    }
    Ok(sum)
}

pub fn div(a: u32, b: u32) -> Result<u32> {
    if b == 0 {
        return Err(Error::DivideByZero);
    }
    Ok(a / b)
}

pub fn eval(a: u32, b: u32, c: u32) -> Result<u32> {
    let sum = add(a, b)?;
    div(sum, c)
}

pub fn eval_or(a: u32, b: u32, c: u32, default: u32) -> u32 {
    match eval(a, b, c) {
        Ok(value) => value,
        Err(_) => default,
    }
}

pub fn half(x: u32) -> Option<u32> {
    if x % 2 == 0 {
        Some(x / 2)
    } else {
        None
    }
}

pub fn quarter(x: u32) -> Option<u32> {
    let h = half(x)?;
    half(h)
}

#[test]
fn test_eval() {
    assert_eq!(3, eval(1, 2, 1).unwrap());
    assert_eq!(Err(Error::DivideByZero), eval(1, 2, 0));
    assert_eq!(Err(Error::Overflow), eval(600, 600, 1));
}

#[test]
fn test_eval_or() {
    assert_eq!(3, eval_or(1, 2, 1, 0));
    assert_eq!(0, eval_or(1, 2, 0, 0));
}

#[test]
fn test_quarter() {
    assert_eq!(Some(2), quarter(8));
    assert_eq!(None, quarter(6));
}
