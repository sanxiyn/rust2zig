const std = @import("std");

fn Option(comptime T: type) type {
    return union(enum) {
        some: T,
        none,

        const Self = @This();

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

pub fn main() void {
    const x: Option(u32) = .{ .some = 42 };
    const y: Option(u32) = .none;
    std.debug.print("{}\n", .{x.isSome()});
    std.debug.print("{}\n", .{y.isSome()});
    std.debug.print("{d}\n", .{x.unwrap()});
}

