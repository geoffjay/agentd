const std = @import("std");
const math = @import("std").math;

/// A fixed-capacity stack of integers.
pub const Stack = struct {
    items: [64]i32,
    len: usize,

    pub fn init() Stack {
        return Stack{ .items = undefined, .len = 0 };
    }

    pub fn push(self: *Stack, item: i32) bool {
        if (self.len >= self.items.len) return false;
        self.items[self.len] = item;
        self.len += 1;
        return true;
    }

    pub fn pop(self: *Stack) ?i32 {
        if (self.len == 0) return null;
        self.len -= 1;
        return self.items[self.len];
    }

    pub fn peek(self: Stack) ?i32 {
        if (self.len == 0) return null;
        return self.items[self.len - 1];
    }
};

/// Cardinal compass directions.
pub const Direction = enum {
    north,
    south,
    east,
    west,
};

/// Result of a parse operation.
pub const ParseError = error{
    InvalidInput,
    Overflow,
    UnexpectedEnd,
};

/// Adds two integers.
pub fn add(a: i32, b: i32) i32 {
    return a + b;
}

/// Returns the absolute value of an integer.
pub fn abs(x: i32) i32 {
    return if (x < 0) -x else x;
}

/// Clamps a value to [lo, hi].
pub fn clamp(value: i32, lo: i32, hi: i32) i32 {
    return math.clamp(value, lo, hi);
}

test "stack push and pop" {
    var s = Stack.init();
    try std.testing.expect(s.push(1));
    try std.testing.expect(s.push(2));
    try std.testing.expectEqual(s.pop(), @as(?i32, 2));
    try std.testing.expectEqual(s.pop(), @as(?i32, 1));
    try std.testing.expectEqual(s.pop(), null);
}

test "add positive numbers" {
    try std.testing.expectEqual(add(2, 3), 5);
}

test "abs negative" {
    try std.testing.expectEqual(abs(-7), 7);
}

test "clamp" {
    try std.testing.expectEqual(clamp(10, 0, 5), 5);
    try std.testing.expectEqual(clamp(-1, 0, 5), 0);
}
