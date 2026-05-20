const std = @import("std");

const Point = struct {
    const Self = @This();

    x: i32,
    y: i32,

    fn translate(self: *Self, dx: i32, dy: i32) void {
        self.x += dx;
        self.y += dy;
    }
};

test "translate" {
    var p: Point = Point{ .x = 1, .y = 2 };
    p.translate(3, 4);
    try std.testing.expectEqual(4, p.x);
    try std.testing.expectEqual(6, p.y);
}

