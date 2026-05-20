pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub fn translate(&mut self, dx: i32, dy: i32) {
        self.x += dx;
        self.y += dy;
    }
}

#[test]
fn test_translate() {
    let mut p = Point { x: 1, y: 2 };
    p.translate(3, 4);
    assert_eq!(4, p.x);
    assert_eq!(6, p.y);
}
