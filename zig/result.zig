const std = @import("std");

fn Result(comptime T: type, comptime E: type) type {
    return union(enum) {
        ok: T,
        err: E,

        const Self = @This();

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

pub fn main() void {
    const x: Result(u32, u32) = .{ .ok = 42 };
    const y: Result(u32, u32) = .{ .err = 1 };
    std.debug.print("{}\n", .{x.isOk()});
    std.debug.print("{}\n", .{y.isErr()});
}

