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

fn vertical(d: Direction) bool {
    return switch (d) {
        .north, .south => true,
        .east, .west => false,
    };
}

test "opposite" {
    try std.testing.expectEqual(.south, opposite(.north));
    try std.testing.expectEqual(.west, opposite(.east));
    try std.testing.expectEqual(.north, opposite(.south));
    try std.testing.expectEqual(.east, opposite(.west));
}

test "vertical" {
    try std.testing.expectEqual(true, vertical(.north));
    try std.testing.expectEqual(false, vertical(.east));
    try std.testing.expectEqual(true, vertical(.south));
    try std.testing.expectEqual(false, vertical(.west));
}
