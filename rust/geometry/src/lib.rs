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

pub enum Shape {
    Dot(Point),
    Line(Point, Point),
    Circle { center: Point, radius: i32 },
}

fn min(a: i32, b: i32) -> i32 {
    if a < b { a } else { b }
}

fn max(a: i32, b: i32) -> i32 {
    if a > b { a } else { b }
}

pub fn bounding_box(s: &Shape) -> (i32, i32, i32, i32) {
    match s {
        Shape::Dot(p) => (p.x, p.y, p.x, p.y),
        Shape::Line(p, q) => (min(p.x, q.x), min(p.y, q.y), max(p.x, q.x), max(p.y, q.y)),
        Shape::Circle { center, radius } => (center.x - radius, center.y - radius, center.x + radius, center.y + radius),
    }
}

#[test]
fn test_translate() {
    let mut p = Point { x: 1, y: 2 };
    p.translate(3, 4);
    assert_eq!(4, p.x);
    assert_eq!(6, p.y);
}

#[test]
fn test_bounding_box_dot() {
    let p = Point { x: 1, y: 2 };
    let (x0, y0, x1, y1) = bounding_box(&Shape::Dot(p));
    assert_eq!(1, x0);
    assert_eq!(2, y0);
    assert_eq!(1, x1);
    assert_eq!(2, y1);
}

#[test]
fn test_bounding_box_line() {
    let p = Point { x: 1, y: 2 };
    let q = Point { x: 2, y: 1 };
    let (x0, y0, x1, y1) = bounding_box(&Shape::Line(p, q));
    assert_eq!(1, x0);
    assert_eq!(1, y0);
    assert_eq!(2, x1);
    assert_eq!(2, y1);
}

#[test]
fn test_bounding_box_circle() {
    let p = Point { x: 2, y: 2 };
    let (x0, y0, x1, y1) = bounding_box(&Shape::Circle { center: p, radius: 1 });
    assert_eq!(1, x0);
    assert_eq!(1, y0);
    assert_eq!(3, x1);
    assert_eq!(3, y1);
}
