const std = @import("std");

pub fn main() void {
    const xs: [5]u32 = .{ 1, 2, 3, 4, 5 };
    var total: u32 = 0;
    for (xs) |x| {
        total += x;
    }
    std.debug.print("{}\n", .{total});
}

