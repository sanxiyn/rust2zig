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

fn consume(_t: Ticket) i32 {
    var t = _t;
    defer t.drop();
    return t.id;
}

fn maybeConsume(c: bool) i32 {
    var t: Ticket = Ticket.new(1);
    var t_alive: bool = true;
    defer {
        if (t_alive) {
            t.drop();
        }
    }
    if (c) {
        t_alive = false;
        return consume(t);
    }
    return t.id;
}

fn ifElse(c: bool) i32 {
    var t: Ticket = Ticket.new(2);
    var t_alive: bool = true;
    defer {
        if (t_alive) {
            t.drop();
        }
    }
    if (c) {
        t_alive = false;
        return consume(t);
    } else {
        return t.id;
    }
}

fn earlyMove(c: bool) Ticket {
    var t: Ticket = Ticket.new(3);
    var t_alive: bool = true;
    defer {
        if (t_alive) {
            t.drop();
        }
    }
    if (c) {
        t_alive = false;
        return t;
    }
    return Ticket.new(30);
}

fn matchMove(c: bool) i32 {
    var t: Ticket = Ticket.new(4);
    var t_alive: bool = true;
    defer {
        if (t_alive) {
            t.drop();
        }
    }
    return switch (c) {
        true => blk: {
            t_alive = false;
            break :blk consume(t);
        },
        false => t.id,
    };
}

fn nestedIf(a: bool, b: bool) i32 {
    var t: Ticket = Ticket.new(5);
    var t_alive: bool = true;
    defer {
        if (t_alive) {
            t.drop();
        }
    }
    if (a) {
        if (b) {
            t_alive = false;
            return consume(t);
        }
        return t.id;
    }
    return t.id;
}

fn twoTickets(c: bool) i32 {
    var a: Ticket = Ticket.new(6);
    var a_alive: bool = true;
    defer {
        if (a_alive) {
            a.drop();
        }
    }
    var b: Ticket = Ticket.new(7);
    var b_alive: bool = true;
    defer {
        if (b_alive) {
            b.drop();
        }
    }
    if (c) {
        a_alive = false;
        _ = consume(a);
        return b.id;
    } else {
        b_alive = false;
        _ = consume(b);
        return a.id;
    }
}

test "maybe_consume_true" {
    resetDrops();
    try std.testing.expectEqual(1, maybeConsume(true));
    try std.testing.expectEqual(1, nDrops());
    try std.testing.expectEqual(1, dropId(0));
}

test "maybe_consume_false" {
    resetDrops();
    try std.testing.expectEqual(1, maybeConsume(false));
    try std.testing.expectEqual(1, nDrops());
    try std.testing.expectEqual(1, dropId(0));
}

test "if_else_true" {
    resetDrops();
    try std.testing.expectEqual(2, ifElse(true));
    try std.testing.expectEqual(1, nDrops());
    try std.testing.expectEqual(2, dropId(0));
}

test "if_else_false" {
    resetDrops();
    try std.testing.expectEqual(2, ifElse(false));
    try std.testing.expectEqual(1, nDrops());
    try std.testing.expectEqual(2, dropId(0));
}

test "early_move_true" {
    resetDrops();
    var t: Ticket = earlyMove(true);
    try std.testing.expectEqual(3, t.id);
    try std.testing.expectEqual(0, nDrops());
    t.drop();
    try std.testing.expectEqual(1, nDrops());
    try std.testing.expectEqual(3, dropId(0));
}

test "early_move_false" {
    resetDrops();
    var t: Ticket = earlyMove(false);
    try std.testing.expectEqual(30, t.id);
    try std.testing.expectEqual(1, nDrops());
    try std.testing.expectEqual(3, dropId(0));
    t.drop();
    try std.testing.expectEqual(2, nDrops());
    try std.testing.expectEqual(30, dropId(1));
}

test "match_move_true" {
    resetDrops();
    try std.testing.expectEqual(4, matchMove(true));
    try std.testing.expectEqual(1, nDrops());
    try std.testing.expectEqual(4, dropId(0));
}

test "match_move_false" {
    resetDrops();
    try std.testing.expectEqual(4, matchMove(false));
    try std.testing.expectEqual(1, nDrops());
    try std.testing.expectEqual(4, dropId(0));
}

test "nested_if" {
    resetDrops();
    try std.testing.expectEqual(5, nestedIf(true, true));
    try std.testing.expectEqual(1, nDrops());
    try std.testing.expectEqual(5, dropId(0));
    resetDrops();
    try std.testing.expectEqual(5, nestedIf(true, false));
    try std.testing.expectEqual(1, nDrops());
    try std.testing.expectEqual(5, dropId(0));
    resetDrops();
    try std.testing.expectEqual(5, nestedIf(false, true));
    try std.testing.expectEqual(1, nDrops());
    try std.testing.expectEqual(5, dropId(0));
}

test "two_tickets_true" {
    resetDrops();
    try std.testing.expectEqual(7, twoTickets(true));
    try std.testing.expectEqual(2, nDrops());
    try std.testing.expectEqual(6, dropId(0));
    try std.testing.expectEqual(7, dropId(1));
}

test "two_tickets_false" {
    resetDrops();
    try std.testing.expectEqual(6, twoTickets(false));
    try std.testing.expectEqual(2, nDrops());
    try std.testing.expectEqual(7, dropId(0));
    try std.testing.expectEqual(6, dropId(1));
}
