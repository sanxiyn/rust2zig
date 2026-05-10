const std = @import("std");

fn divmod(a: u32, b: u32) struct { u32, u32 } {
    return .{ a / b, a % b };
}

test "divmod" {
    const q, const r = divmod(7, 3);
    try std.testing.expectEqual(2, q);
    try std.testing.expectEqual(1, r);
}

