const std = @import("std");

const Error = error{ Overflow, DivideByZero };
const limit: u32 = 1000;

fn add(a: u32, b: u32) Error!u32 {
    const sum: u32 = a + b;
    if (sum > limit) {
        return error.Overflow;
    }
    return sum;
}

fn div(a: u32, b: u32) Error!u32 {
    if (b == 0) {
        return error.DivideByZero;
    }
    return a / b;
}

fn eval(a: u32, b: u32, c: u32) Error!u32 {
    const sum: u32 = try add(a, b);
    return div(sum, c);
}

fn evalChain(a: u32, b: u32, c: u32) Error!u32 {
    return div(try add(a, b), c);
}

fn evalOr(a: u32, b: u32, c: u32, default: u32) u32 {
    return if (eval(a, b, c)) |value| value else |_| default;
}

fn half(x: u32) ?u32 {
    if (x % 2 == 0) {
        return x / 2;
    } else {
        return null;
    }
}

fn quarter(x: u32) ?u32 {
    const h: u32 = half(x) orelse return null;
    return half(h);
}

test "eval" {
    try std.testing.expectEqual(3, try eval(1, 2, 1));
    try std.testing.expectEqual(error.DivideByZero, eval(1, 2, 0));
    try std.testing.expectEqual(error.Overflow, eval(600, 600, 1));
}

test "eval_chain" {
    try std.testing.expectEqual(3, try evalChain(1, 2, 1));
    try std.testing.expectEqual(error.DivideByZero, evalChain(1, 2, 0));
}

test "eval_or" {
    try std.testing.expectEqual(3, evalOr(1, 2, 1, 0));
    try std.testing.expectEqual(0, evalOr(1, 2, 0, 0));
}

test "quarter" {
    try std.testing.expectEqual(2, quarter(8));
    try std.testing.expectEqual(null, quarter(6));
}
