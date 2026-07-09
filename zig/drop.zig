const std = @import("std");

var dropLog: [16]i32 = .{0} ** 16;

var dropLen: usize = 0;

fn logDrop(id: i32) void {
    const i: usize = dropLen;
    if (i < 16) {
        dropLog[i] = id;
        dropLen = i + 1;
    }
}

fn resetDrops() void {
    dropLen = 0;
}

fn nDrops() usize {
    return dropLen;
}

fn dropId(i: usize) i32 {
    return dropLog[i];
}

const Ticket = struct {
    const Self = @This();

    id: i32,

    fn drop(self: *Self) void {
        logDrop(self.id);
    }

    fn new(id: i32) Ticket {
        return Ticket{ .id = id };
    }
};

fn scopeEnd() i32 {
    var t: Ticket = Ticket.new(1);
    defer t.drop();
    return t.id;
}

fn earlyReturn(early: bool) i32 {
    var t: Ticket = Ticket.new(2);
    defer t.drop();
    if (early) {
        return t.id;
    }
    return t.id;
}

fn moveOut() Ticket {
    const t: Ticket = Ticket.new(3);
    return t;
}

fn consume(_t: Ticket) i32 {
    var t = _t;
    defer t.drop();
    return t.id;
}

fn forward(t: Ticket) Ticket {
    return t;
}

fn nested() i32 {
    var outer: Ticket = Ticket.new(4);
    defer outer.drop();
    const n: i32 = blk: {
        var inner: Ticket = Ticket.new(5);
        defer inner.drop();
        break :blk inner.id;
    };
    return outer.id + n;
}

fn dropOrder() void {
    var a: Ticket = Ticket.new(6);
    defer a.drop();
    var b: Ticket = Ticket.new(7);
    defer b.drop();
    _ = a.id + b.id;
}

test "scope_end" {
    resetDrops();
    try std.testing.expectEqual(1, scopeEnd());
    try std.testing.expectEqual(1, nDrops());
    try std.testing.expectEqual(1, dropId(0));
}

test "early_return_true" {
    resetDrops();
    try std.testing.expectEqual(2, earlyReturn(true));
    try std.testing.expectEqual(1, nDrops());
    try std.testing.expectEqual(2, dropId(0));
}

test "early_return_false" {
    resetDrops();
    try std.testing.expectEqual(2, earlyReturn(false));
    try std.testing.expectEqual(1, nDrops());
    try std.testing.expectEqual(2, dropId(0));
}

test "move_out" {
    resetDrops();
    var t: Ticket = moveOut();
    try std.testing.expectEqual(3, t.id);
    try std.testing.expectEqual(0, nDrops());
    t.drop();
    try std.testing.expectEqual(1, nDrops());
    try std.testing.expectEqual(3, dropId(0));
}

test "consume" {
    resetDrops();
    try std.testing.expectEqual(8, consume(Ticket.new(8)));
    try std.testing.expectEqual(1, nDrops());
    try std.testing.expectEqual(8, dropId(0));
}

test "forward" {
    resetDrops();
    var t: Ticket = forward(Ticket.new(9));
    try std.testing.expectEqual(0, nDrops());
    try std.testing.expectEqual(9, t.id);
    t.drop();
    try std.testing.expectEqual(1, nDrops());
    try std.testing.expectEqual(9, dropId(0));
}

test "nested" {
    resetDrops();
    try std.testing.expectEqual(9, nested());
    try std.testing.expectEqual(2, nDrops());
    try std.testing.expectEqual(5, dropId(0));
    try std.testing.expectEqual(4, dropId(1));
}

test "drop_order" {
    resetDrops();
    dropOrder();
    try std.testing.expectEqual(2, nDrops());
    try std.testing.expectEqual(7, dropId(0));
    try std.testing.expectEqual(6, dropId(1));
}
