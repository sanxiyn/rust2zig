const std = @import("std");

const BitSet = struct {
    const Self = @This();

    data: usize,
    length: usize,

    fn withCapacity(bits: usize) Self {
        return BitSet{ .data = 0, .length = bits };
    }

    fn contains(self: *const Self, bit: usize) bool {
        return bit < self.length and self.data & (1 << bit) != 0;
    }

    fn put(self: *Self, bit: usize) bool {
        std.debug.assert(bit < self.length);
        const prev: bool = self.data & (1 << bit) != 0;
        self.data |= 1 << bit;
        return prev;
    }

    fn toggle(self: *Self, bit: usize) void {
        std.debug.assert(bit < self.length);
        self.data ^= 1 << bit;
    }
};

test "toggle" {
    var b: BitSet = BitSet.withCapacity(16);
    b.toggle(1);
    _ = b.put(2);
    b.toggle(2);
    _ = b.put(3);
    std.debug.assert(b.contains(1));
    std.debug.assert(!b.contains(2));
    std.debug.assert(b.contains(3));
}
