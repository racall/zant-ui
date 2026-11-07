pub const Theme = struct {
    color_primary: u32 = 0xff1677ff,
    color_primary_hover: u32 = 0xff4096ff,
    color_primary_active: u32 = 0xff0958d9,
    color_primary_bg: u32 = 0xffe6f4ff,

    color_text: u32 = 0xe0000000,
    color_text_secondary: u32 = 0xa6000000,
    color_text_disabled: u32 = 0x40000000,
    color_text_light: u32 = 0xffffffff,

    color_bg_base: u32 = 0xfff5f5f5,
    color_bg_container: u32 = 0xffffffff,
    color_bg_elevated: u32 = 0xffffffff,
    /// antd colorBgLayout: page background, segmented track.
    color_bg_layout: u32 = 0xfff5f5f5,
    // antd 5 fills are translucent so they read the same on any surface
    color_fill: u32 = 0x26000000, // rgba(0,0,0,.15) — pressed
    color_fill_secondary: u32 = 0x0f000000, // rgba(0,0,0,.06) — hover
    color_fill_tertiary: u32 = 0x0a000000, // rgba(0,0,0,.04)
    color_fill_quaternary: u32 = 0x05000000, // rgba(0,0,0,.02)

    color_border: u32 = 0xffd9d9d9,
    color_border_secondary: u32 = 0xfff0f0f0,
    color_split: u32 = 0xfff0f0f0,

    color_success: u32 = 0xff52c41a,
    color_warning: u32 = 0xfffaad14,
    color_error: u32 = 0xffff4d4f,
    // status surfaces (antd colorSuccessBg… / the Tag presets' text, which
    // stays readable on them)
    color_success_bg: u32 = 0xfff6ffed,
    color_success_text: u32 = 0xff389e0d,
    color_warning_bg: u32 = 0xfffffbe6,
    color_warning_text: u32 = 0xffd48806,
    color_error_bg: u32 = 0xfffff2f0,
    color_error_text: u32 = 0xffcf1322,

    border_radius: f32 = 6,
    border_radius_sm: f32 = 4,
    border_radius_lg: f32 = 8,

    font_size: f32 = 14,
    font_size_sm: f32 = 12,
    font_size_lg: f32 = 16,

    control_height: f32 = 32,
    control_height_sm: f32 = 24,
    control_height_lg: f32 = 40,

    padding: f32 = 16,
    padding_sm: f32 = 8,
    padding_lg: f32 = 24,

    // antd motionDurationFast / Mid / Slow
    motion_duration_fast: u32 = 100,
    motion_duration_mid: u32 = 200,
    motion_duration_slow: u32 = 300,

    control_padding_horizontal: f32 = 15,
    control_padding_horizontal_sm: f32 = 7,
    control_padding_horizontal_lg: f32 = 15,
};

pub const ThemeMode = enum {
    light,
    dark,
};

pub fn antLight() Theme {
    return .{};
}

/// Dark counterpart of `antLight` (Ant Design dark algorithm values).
pub fn antDark() Theme {
    return .{
        .color_primary = 0xff1668dc,
        .color_primary_hover = 0xff3c89e8,
        .color_primary_active = 0xff1554ad,
        .color_primary_bg = 0xff111a2c,
        .color_text = 0xd9ffffff,
        .color_text_secondary = 0xa6ffffff,
        .color_text_disabled = 0x40ffffff,
        .color_text_light = 0xffffffff,
        .color_bg_base = 0xff000000,
        .color_bg_container = 0xff141414,
        .color_bg_elevated = 0xff1f1f1f,
        .color_bg_layout = 0xff000000,
        .color_fill = 0x2effffff,
        .color_fill_secondary = 0x1fffffff,
        .color_fill_tertiary = 0x14ffffff,
        .color_fill_quaternary = 0x0affffff,
        .color_border = 0xff424242,
        .color_border_secondary = 0xff303030,
        .color_split = 0x0fffffff,
        .color_success_bg = 0xff162312,
        .color_success_text = 0xff49aa19,
        .color_warning_bg = 0xff2b2111,
        .color_warning_text = 0xffd89614,
        .color_error_bg = 0xff2c1618,
        .color_error_text = 0xffdc4446,
    };
}

pub fn forMode(mode: ThemeMode) Theme {
    return switch (mode) {
        .light => antLight(),
        .dark => antDark(),
    };
}

pub fn rgba(r: u8, g: u8, b: u8, a: u8) u32 {
    return (@as(u32, a) << 24) | (@as(u32, r) << 16) | (@as(u32, g) << 8) | b;
}

pub fn alpha(color: u32) u8 {
    return @intCast((color >> 24) & 0xff);
}

pub fn red(color: u32) u8 {
    return @intCast((color >> 16) & 0xff);
}

pub fn green(color: u32) u8 {
    return @intCast((color >> 8) & 0xff);
}

pub fn blue(color: u32) u8 {
    return @intCast(color & 0xff);
}

pub fn lerp(a: u32, b: u32, t: f32) u32 {
    const k = @max(0, @min(1, t));
    const ia: i32 = @intCast(alpha(a));
    const ib: i32 = @intCast(alpha(b));
    const r = @as(f32, @floatFromInt(red(a))) + (@as(f32, @floatFromInt(red(b))) - @as(f32, @floatFromInt(red(a)))) * k;
    const g = @as(f32, @floatFromInt(green(a))) + (@as(f32, @floatFromInt(green(b))) - @as(f32, @floatFromInt(green(a)))) * k;
    const bl = @as(f32, @floatFromInt(blue(a))) + (@as(f32, @floatFromInt(blue(b))) - @as(f32, @floatFromInt(blue(a)))) * k;
    const al = @as(f32, @floatFromInt(ia)) + (@as(f32, @floatFromInt(ib)) - @as(f32, @floatFromInt(ia))) * k;
    return (@as(u32, @intFromFloat(@max(0, @min(255, al)))) << 24) |
        (@as(u32, @intFromFloat(@max(0, @min(255, r)))) << 16) |
        (@as(u32, @intFromFloat(@max(0, @min(255, g)))) << 8) |
        @as(u32, @intFromFloat(@max(0, @min(255, bl))));
}

pub fn get(c: u32) u32 {
    return c;
}

pub fn withAlpha(color: u32, a: u8) u32 {
    return (color & 0x00ffffff) | (@as(u32, a) << 24);
}
