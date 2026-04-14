const std = @import("std");

fn div(a: u32, b: u32) ?u32 {
    if (a % b == 0) {
        return a / b;
    } else {
        return null;
    }
}

pub fn main() void {
    if (div(6, 3)) |x| {
        std.debug.print("{}\n", .{x});
    } else {
        std.debug.print("not divisible\n", .{});
    }
    if (div(7, 3)) |x| {
        std.debug.print("{}\n", .{x});
    } else {
        std.debug.print("not divisible\n", .{});
    }
}

