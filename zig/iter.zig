const std = @import("std");

fn position(l: []const i32, v: i32) ?usize {
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
    const l: [5]i32 = .{ 1, 2, 3, 4, 5 };
    try std.testing.expectEqual(2, position(&l, 3));
    try std.testing.expectEqual(null, position(&l, 6));
}

