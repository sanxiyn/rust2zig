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

fn min(a: i32, b: i32) i32 {
    if (a < b) {
        return a;
    } else {
        return b;
    }
}

fn max(a: i32, b: i32) i32 {
    if (a > b) {
        return a;
    } else {
        return b;
    }
}

fn boundingBox(s: Shape) struct { i32, i32, i32, i32 } {
    return switch (s) {
        .dot => |p| .{ p.x, p.y, p.x, p.y },
        .line => |_line| blk: {
            const p = _line[0];
            const q = _line[1];
            break :blk .{ min(p.x, q.x), min(p.y, q.y), max(p.x, q.x), max(p.y, q.y) };
        },
        .circle => |_circle| blk: {
            const center = _circle.center;
            const radius = _circle.radius;
            break :blk .{ center.x - radius, center.y - radius, center.x + radius, center.y + radius };
        },
    };
}

test "translate" {
    const p: Point = Point{ .x = 1, .y = 2 };
    const q: Point = p.translate(3, 4);
    try std.testing.expectEqual(4, q.x);
    try std.testing.expectEqual(6, q.y);
}

test "bounding_box_dot" {
    const p: Point = Point{ .x = 1, .y = 2 };
    const x0, const y0, const x1, const y1 = boundingBox(.{ .dot = p });
    try std.testing.expectEqual(1, x0);
    try std.testing.expectEqual(2, y0);
    try std.testing.expectEqual(1, x1);
    try std.testing.expectEqual(2, y1);
}

test "bounding_box_line" {
    const p: Point = Point{ .x = 1, .y = 2 };
    const q: Point = Point{ .x = 2, .y = 1 };
    const x0, const y0, const x1, const y1 = boundingBox(.{ .line = .{ p, q } });
    try std.testing.expectEqual(1, x0);
    try std.testing.expectEqual(1, y0);
    try std.testing.expectEqual(2, x1);
    try std.testing.expectEqual(2, y1);
}

test "bounding_box_circle" {
    const p: Point = Point{ .x = 2, .y = 2 };
    const x0, const y0, const x1, const y1 = boundingBox(.{ .circle = .{ .center = p, .radius = 1 } });
    try std.testing.expectEqual(1, x0);
    try std.testing.expectEqual(1, y0);
    try std.testing.expectEqual(3, x1);
    try std.testing.expectEqual(3, y1);
}

