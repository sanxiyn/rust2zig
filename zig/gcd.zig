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

pub fn main() void {
    const result: u32 = gcd(16, 10);
    std.debug.print("{}\n", .{result});
}

