const std = @import("std");

fn inc(x: *i32) void {
    x.* += 1;
}

fn succ(x: *const i32) i32 {
    return x.* + 1;
}

test "inc" {
    var x: i32 = 2;
    const y: i32 = x;
    inc(&x);
    try std.testing.expectEqual(succ(&y), x);
}
