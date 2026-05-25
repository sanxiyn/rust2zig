const std = @import("std");

test "closure" {
    const x: i32 = 3;
    const double = struct {
        fn call(_: @This(), x2: i32) i32 {
            return x2 * 2;
        }
    }{};
    try std.testing.expectEqual(6, double.call(x));
}

test "capture" {
    const a: i32 = 3;
    const add = struct {
        a: i32,
        fn call(self: @This(), x: i32) i32 {
            return x + self.a;
        }
    }{ .a = a };
    try std.testing.expectEqual(6, add.call(3));
}

