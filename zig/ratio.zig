const std = @import("std");

fn gcd(_a: u32, _b: u32) u32 {
    var a = _a;
    var b = _b;
    while (b != 0) {
        const t = b;
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
        const n = self.num * other.denom + other.num * self.denom;
        const d = self.denom * other.denom;
        const g = gcd(n, d);
        return Ratio{ .num = n / g, .denom = d / g };
    }
};

pub fn main() void {
    const a = Ratio{ .num = 1, .denom = 2 };
    const b = Ratio{ .num = 1, .denom = 3 };
    const c = a.add(b);
    std.debug.print("{}/{}\n", .{ c.num, c.denom });
    const d = b.add(b);
    std.debug.print("{}/{}\n", .{ d.num, d.denom });
}

