const std = @import("std");

const Rand32 = struct {
    const Self = @This();

    const defaultInc: u64 = 1442695040888963407;
    const multiplier: u64 = 6364136223846793005;

    state: u64,
    inc: u64,

    fn new(seed: u64) Self {
        return Self.newInc(seed, defaultInc);
    }

    fn newInc(seed: u64, increment: u64) Self {
        var rng: Rand32 = Self{ .state = 0, .inc = increment << @intCast(1) | 1 };
        _ = rng.randU32();
        rng.state +%= seed;
        _ = rng.randU32();
        return rng;
    }

    fn randU32(self: *Self) u32 {
        const oldstate: u64 = self.state;
        self.state = oldstate *% multiplier +% self.inc;
        const xorshifted: u32 = @truncate(((oldstate >> @intCast(18)) ^ oldstate) >> @intCast(27));
        const rot: u32 = @truncate(oldstate >> @intCast(59));
        return std.math.rotr(u32, xorshifted, rot);
    }
};

test "rand32" {
    const seed: u64 = 54321;
    var r1: Rand32 = Rand32.new(seed);
    try std.testing.expectEqual(2891073575, r1.randU32());
}
