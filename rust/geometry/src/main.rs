#[derive(Clone, Copy)]
struct Point {
    x: i32,
    y: i32,
}

impl Point {
    fn translate(self, dx: i32, dy: i32) -> Point {
        Point { x: self.x + dx, y: self.y + dy }
    }
}

enum Shape {
    Dot(Point),
    Line(Point, Point),
    Circle { center: Point, radius: i32 },
}

fn describe(s: Shape) {
    match s {
        Shape::Dot(p) => println!("dot {} {}", p.x, p.y),
        Shape::Line(p, q) => println!("line {} {} {} {}", p.x, p.y, q.x, q.y),
        Shape::Circle { center, radius } => println!("circle {} {} {}", center.x, center.y, radius),
    }
}

fn main() {
    let p = Point { x: 1, y: 2 };
    let q = p.translate(3, 4);
    println!("{} {}", q.x, q.y);
    describe(Shape::Dot(p));
    describe(Shape::Line(p, q));
    describe(Shape::Circle { center: p, radius: 5 });
}
