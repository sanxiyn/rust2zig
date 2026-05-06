const std = @import("std");

fn sum(xs: []const i32) i32 {
    var total: i32 = 0;
    for (xs) |x| {
        total += x;
    }
    return total;
}

pub fn main() void {
    const xs: [5]i32 = .{ 1, 2, 3, 4, 5 };
    var total: i32 = 0;
    for (xs) |x| {
        total += x;
    }
    std.debug.print("{}\n", .{total});
    std.debug.print("{}\n", .{sum(&xs)});
    total = 0;
    for (1..6) |_x| {
        const x: i32 = @intCast(_x);
        total += x;
    }
    std.debug.print("{}\n", .{total});
}

