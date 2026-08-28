const std = @import("std");

fn notBool(b: bool) bool {
    return !b;
}

fn notInt(x: u8) u8 {
    return ~x;
}

test "not" {
    try std.testing.expectEqual(false, notBool(true));
    try std.testing.expectEqual(true, notBool(false));
    try std.testing.expectEqual(0xf0, notInt(0x0f));
    try std.testing.expectEqual(255, notInt(0));
}
