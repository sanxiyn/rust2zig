const std = @import("std");

fn div(a: u32, b: u32) ?u32 {
    if (a % b == 0) {
        return a / b;
    } else {
        return null;
    }
}

test "div" {
    try std.testing.expectEqual(2, div(6, 3));
    try std.testing.expectEqual(null, div(7, 3));
}

