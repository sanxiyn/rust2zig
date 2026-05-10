const std = @import("std");

const Direction = enum {
    north,
    east,
    south,
    west,
};

fn opposite(d: Direction) Direction {
    return switch (d) {
        .north => .south,
        .east => .west,
        .south => .north,
        .west => .east,
    };
}

test "direction" {
    try std.testing.expectEqual(.south, opposite(.north));
    try std.testing.expectEqual(.west, opposite(.east));
    try std.testing.expectEqual(.north, opposite(.south));
    try std.testing.expectEqual(.east, opposite(.west));
}

