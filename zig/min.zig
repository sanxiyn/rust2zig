const std = @import("std");

fn min(comptime T: type, a: T, b: T) T {
    if (a < b) {
        return a;
    } else {
        return b;
    }
}

test "min" {
    const a: i32 = 2;
    const b: i32 = 3;
    try std.testing.expectEqual(2, min(i32, a, b));
    try std.testing.expectEqual(2, min(i32, b, a));
}

