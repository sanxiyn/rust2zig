pub struct Point {
    pub x: i32,
    pub y: i32,
}

pub struct Size {
    pub width: i32,
    pub height: i32,
}

pub struct Rect {
    pub origin: Point,
    pub size: Size,
}

impl Rect {
    pub fn min(&self) -> Point {
        let x = self.origin.x;
        let y = self.origin.y;
        Point { x, y }
    }

    pub fn max(&self) -> Point {
        let x = self.origin.x + self.size.width;
        let y = self.origin.y + self.size.height;
        Point { x, y }
    }

    pub fn to_box(&self) -> Box {
        let min = self.min();
        let max = self.max();
        Box { min, max }
    }

    pub fn contains(&self, p: Point) -> bool {
        self.to_box().contains(p)
    }
}

pub struct Box {
    pub min: Point,
    pub max: Point,
}

impl Box {
    pub fn contains(&self, p: Point) -> bool {
        (self.min.x <= p.x) && (p.x < self.max.x) && (self.min.y <= p.y) && (p.y < self.max.y)
    }
}

#[test]
fn test_contains() {
    let origin = Point { x: 1, y: 2 };
    let size = Size { width: 3, height: 4 };
    let rect = Rect { origin, size };
    assert!(!rect.contains(Point { x: 0, y: 0 }));
    assert!(!rect.contains(Point { x: 0, y: 3 }));
    assert!(!rect.contains(Point { x: 0, y: 7 }));
    assert!(!rect.contains(Point { x: 3, y: 0 }));
    assert!(rect.contains(Point { x: 3, y: 3 }));
    assert!(!rect.contains(Point { x: 3, y: 7 }));
    assert!(!rect.contains(Point { x: 7, y: 0 }));
    assert!(!rect.contains(Point { x: 7, y: 3 }));
    assert!(!rect.contains(Point { x: 7, y: 7 }));
}
