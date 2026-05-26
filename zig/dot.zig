const std = @import("std");

fn dot(a: []const i32, b: []const i32) i32 {
    var sum: i32 = 0;
    for (a, b) |*x, *y| {
        sum += x.* * y.*;
    }
    return sum;
}

test "dot" {
    const a: [3]i32 = .{ 1, 2, 3 };
    const b: [3]i32 = .{ 4, 5, 6 };
    try std.testing.expectEqual(32, dot(&a, &b));
}

