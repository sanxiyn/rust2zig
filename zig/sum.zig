const std = @import("std");

fn sum(xs: []const i32) i32 {
    var total: i32 = 0;
    for (xs) |*x| {
        total += x.*;
    }
    return total;
}

fn sum2(xs: []const i32) i32 {
    var total: i32 = 0;
    for (0..xs.len) |_i| {
        const i: usize = @intCast(_i);
        total += xs[i];
    }
    return total;
}

fn sumOdd(xs: []const i32) i32 {
    var total: i32 = 0;
    for (xs) |*x| {
        if (@rem(x.*, 2) == 0) {
            continue;
        }
        total += x.*;
    }
    return total;
}

test "sum" {
    const xs: [5]i32 = .{ 1, 2, 3, 4, 5 };
    var total: i32 = 0;
    for (xs) |x| {
        total += x;
    }
    try std.testing.expectEqual(15, total);
    try std.testing.expectEqual(15, sum(&xs));
    total = 0;
    for (1..6) |_x| {
        const x: i32 = @intCast(_x);
        total += x;
    }
    try std.testing.expectEqual(15, total);
}

test "sum2" {
    const xs: [5]i32 = .{ 1, 2, 3, 4, 5 };
    try std.testing.expectEqual(15, sum2(&xs));
}

test "sum_odd" {
    const xs: [5]i32 = .{ 1, 2, 3, 4, 5 };
    try std.testing.expectEqual(9, sumOdd(&xs));
}

