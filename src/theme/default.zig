const tokens = @import("tokens.zig");

pub const Theme = tokens.Theme;

pub fn light() Theme {
    return tokens.antLight();
}
