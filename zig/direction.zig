const std = @import("std");

const Direction = enum {
    north,
    east,
    south,
    west,
};

fn toString(d: Direction) []const u8 {
    return switch (d) {
        .north => "North",
        .east => "East",
        .south => "South",
        .west => "West",
    };
}

fn opposite(d: Direction) Direction {
    return switch (d) {
        .north => .south,
        .east => .west,
        .south => .north,
        .west => .east,
    };
}

pub fn main() void {
    std.debug.print("{s}\n", .{toString(opposite(.north))});
    std.debug.print("{s}\n", .{toString(opposite(.east))});
    std.debug.print("{s}\n", .{toString(opposite(.south))});
    std.debug.print("{s}\n", .{toString(opposite(.west))});
}

