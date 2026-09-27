pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[test]
fn test_translate() {
    let p = Point { x: 1, y: 2 };
    let Point { mut x, y } = p;
    x += 3;
    let y = y + 4;
    assert_eq!(4, x);
    assert_eq!(6, y);
}
