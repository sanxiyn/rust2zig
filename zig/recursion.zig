const std = @import("std");

fn fib(n: u32) u32 {
    if (n < 2) {
        return n;
    }
    return fib(n - 1) + fib(n - 2);
}

fn isEven(n: u32) bool {
    if (n == 0) {
        return true;
    }
    return isOdd(n - 1);
}

fn isOdd(n: u32) bool {
    if (n == 0) {
        return false;
    }
    return isEven(n - 1);
}

test "fib" {
    try std.testing.expectEqual(55, fib(10));
}

test "parity" {
    try std.testing.expectEqual(true, isEven(10));
    try std.testing.expectEqual(false, isOdd(10));
}
