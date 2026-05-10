const std = @import("std");

test "closure" {
    const i: i32 = 3;
    const double = struct {
        fn call(x: i32) i32 {
            return x * 2;
        }
    }.call;
    try std.testing.expectEqual(6, double(i));
}

