const std = @import("std");

fn position(comptime T: type, l: []const T, v: T) ?usize {
    var i: usize = 0;
    for (l) |*e| {
        if (e.* == v) {
            break;
        }
        i += 1;
    }
    if (i == l.len) {
        return null;
    } else {
        return i;
    }
}

test "position" {
    const l: []const i32 = &.{ 1, 2, 3, 4, 5 };
    const v: i32 = 3;
    try std.testing.expectEqual(2, position(i32, l, v));
    const v2: i32 = 6;
    try std.testing.expectEqual(null, position(i32, l, v2));
}

