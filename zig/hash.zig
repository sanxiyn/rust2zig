const std = @import("std");

const fnvOffsetBasis32: u32 = 0x811c9dc5;
const fnvPrime32: u32 = 0x01000193;

fn fnv1aHash32(bytes: []const u8, limit: ?usize) u32 {
    const prime: u32 = fnvPrime32;
    var hash: u32 = fnvOffsetBasis32;
    var i: usize = 0;
    const len: usize = blk: {
        if (limit) |v| {
            if (0 < v and v < bytes.len) {
                break :blk v;
            }
        }
        break :blk bytes.len;
    };
    while (i < len) {
        hash ^= @as(u32, bytes[i]);
        hash *%= prime;
        i += 1;
    }
    return hash;
}

fn fnv1aHashStr32(input: []const u8) u32 {
    return fnv1aHash32(input, null);
}

const foobar: []const u8 = "foobar";
const foobarHash32: u32 = 0xbf9cf968;

test "32" {
    const hashed: u32 = fnv1aHashStr32(foobar);
    try std.testing.expectEqual(foobarHash32, hashed);
}
