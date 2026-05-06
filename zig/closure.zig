const std = @import("std");

pub fn main() void {
    const i: i32 = 3;
    const double = struct {
        fn call(x: i32) i32 {
            return x * 2;
        }
    }.call;
    std.debug.print("{}\n", .{double(i)});
}

