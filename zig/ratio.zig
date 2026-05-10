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

const Ratio = struct {
    const Self = @This();

    num: u32,
    denom: u32,

    fn add(self: Self, other: Ratio) Ratio {
        const n: u32 = self.num * other.denom + other.num * self.denom;
        const d: u32 = self.denom * other.denom;
        const g: u32 = gcd(n, d);
        return Ratio{ .num = n / g, .denom = d / g };
    }
};

test "ratio" {
    const a: Ratio = Ratio{ .num = 1, .denom = 2 };
    const b: Ratio = Ratio{ .num = 1, .denom = 3 };
    const c: Ratio = a.add(b);
    try std.testing.expectEqual(5, c.num);
    try std.testing.expectEqual(6, c.denom);
    const d: Ratio = b.add(b);
    try std.testing.expectEqual(2, d.num);
    try std.testing.expectEqual(3, d.denom);
}

