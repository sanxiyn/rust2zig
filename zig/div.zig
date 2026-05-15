const std = @import("std");

fn div(a: u32, b: u32) ?u32 {
    if (a % b == 0) {
        return a / b;
    } else {
        return null;
    }
}

fn div2(a: u32, b: u32) u32 {
    if (div(a, b)) |x| {
        return x;
    } else {
        return 0;
    }
}

test "div" {
    try std.testing.expectEqual(2, div(6, 3));
    try std.testing.expectEqual(null, div(7, 3));
}

test "div2" {
    try std.testing.expectEqual(2, div2(6, 3));
    try std.testing.expectEqual(0, div2(7, 3));
}

