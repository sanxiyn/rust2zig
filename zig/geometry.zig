const std = @import("std");

const Point = struct {
    const Self = @This();

    x: i32,
    y: i32,

    fn translate(self: Self, dx: i32, dy: i32) Point {
        return Point{ .x = self.x + dx, .y = self.y + dy };
    }
};

const Shape = union(enum) {
    dot: Point,
    line: struct { Point, Point },
    circle: struct { center: Point, radius: i32 },
};

fn describe(s: Shape) void {
    return switch (s) {
        .dot => |p| std.debug.print("dot {} {}\n", .{ p.x, p.y }),
        .line => |_line| {
            const p = _line[0];
            const q = _line[1];
            std.debug.print("line {} {} {} {}\n", .{ p.x, p.y, q.x, q.y });
        },
        .circle => |_circle| {
            const center = _circle.center;
            const radius = _circle.radius;
            std.debug.print("circle {} {} {}\n", .{ center.x, center.y, radius });
        },
    };
}

pub fn main() void {
    const p: Point = Point{ .x = 1, .y = 2 };
    const q: Point = p.translate(3, 4);
    std.debug.print("{} {}\n", .{ q.x, q.y });
    describe(.{ .dot = p });
    describe(.{ .line = .{ p, q } });
    describe(.{ .circle = .{ .center = p, .radius = 5 } });
}

