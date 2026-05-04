const std = @import("std");

fn divmod(a: u32, b: u32) struct { u32, u32 } {
    return .{ a / b, a % b };
}

pub fn main() void {
    const q, const r = divmod(7, 3);
    std.debug.print("{} {}\n", .{ q, r });
}

