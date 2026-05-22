const std = @import("std");

test "closure" {
    const x: i32 = 3;
    const double = struct {
        fn call(x2: i32) i32 {
            return x2 * 2;
        }
    }.call;
    try std.testing.expectEqual(6, double(x));
}

