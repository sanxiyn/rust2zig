const std = @import("std");

fn Option(comptime T: type) type {
    return union(enum) {
        const Self = @This();

        some: T,
        none,

        fn @"and"(self: Self, comptime U: type, optb: Option(U)) Option(U) {
            return switch (self) {
                .some => optb,
                .none => .none,
            };
        }

        fn isNone(self: Self) bool {
            return switch (self) {
                .some => false,
                .none => true,
            };
        }

        fn isSome(self: Self) bool {
            return switch (self) {
                .some => true,
                .none => false,
            };
        }

        fn unwrap(self: Self) T {
            return switch (self) {
                .some => |x| x,
                .none => @panic("called unwrap on None"),
            };
        }
    };
}

test "option" {
    const x: Option(u32) = .{ .some = 42 };
    const y: Option(u32) = .none;
    try std.testing.expectEqual(true, x.isSome());
    try std.testing.expectEqual(false, y.isSome());
    try std.testing.expectEqual(42, x.unwrap());
    const z: Option(i32) = .{ .some = 7 };
    try std.testing.expectEqual(true, y.@"and"(i32, z).isNone());
}

