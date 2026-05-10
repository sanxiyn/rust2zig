const std = @import("std");

fn gcd(_a: u32, _b: u32) u32 {
    var a = _a;
    var b = _b;
    while (b != 0) {
        const t: u32 = b;
        b = a % b;
        a = t;
    }
    return a;
}

test "gcd" {
    try std.testing.expectEqual(2, gcd(16, 10));
}

