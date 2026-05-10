const std = @import("std");

fn Result(comptime T: type, comptime E: type) type {
    return union(enum) {
        const Self = @This();

        ok: T,
        err: E,

        fn isOk(self: Self) bool {
            return switch (self) {
                .ok => true,
                .err => false,
            };
        }

        fn isErr(self: Self) bool {
            return switch (self) {
                .ok => false,
                .err => true,
            };
        }
    };
}

test "result" {
    const x: Result(u32, u32) = .{ .ok = 42 };
    const y: Result(u32, u32) = .{ .err = 1 };
    try std.testing.expectEqual(true, x.isOk());
    try std.testing.expectEqual(true, y.isErr());
}

