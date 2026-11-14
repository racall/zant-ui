//! Ant Design 5 design tokens (light / dark)
//! and the ARGB colour helpers used across the framework.
#![forbid(unsafe_code)]

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub color_primary: u32,
    pub color_primary_hover: u32,
    pub color_primary_active: u32,
    pub color_primary_bg: u32,

    pub color_text: u32,
    pub color_text_secondary: u32,
    pub color_text_disabled: u32,
    pub color_text_light: u32,

    pub color_bg_base: u32,
    pub color_bg_container: u32,
    pub color_bg_elevated: u32,
    /// antd colorBgLayout: page background, segmented track.
    pub color_bg_layout: u32,
    // antd 5 fills are translucent so they read the same on any surface
    pub color_fill: u32,            // rgba(0,0,0,.15) — pressed
    pub color_fill_secondary: u32,  // rgba(0,0,0,.06) — hover
    pub color_fill_tertiary: u32,   // rgba(0,0,0,.04)
    pub color_fill_quaternary: u32, // rgba(0,0,0,.02)

    pub color_border: u32,
    pub color_border_secondary: u32,
    pub color_split: u32,

    pub color_success: u32,
    pub color_warning: u32,
    pub color_error: u32,
    // status surfaces (antd colorSuccessBg… / the Tag presets' text, which
    // stays readable on them)
    pub color_success_bg: u32,
    pub color_success_text: u32,
    pub color_warning_bg: u32,
    pub color_warning_text: u32,
    pub color_error_bg: u32,
    pub color_error_text: u32,

    pub border_radius: f32,
    pub border_radius_sm: f32,
    pub border_radius_lg: f32,

    pub font_size: f32,
    pub font_size_sm: f32,
    pub font_size_lg: f32,

    pub control_height: f32,
    pub control_height_sm: f32,
    pub control_height_lg: f32,

    pub padding: f32,
    pub padding_sm: f32,
    pub padding_lg: f32,

    // antd motionDurationFast / Mid / Slow
    pub motion_duration_fast: u32,
    pub motion_duration_mid: u32,
    pub motion_duration_slow: u32,

    pub control_padding_horizontal: f32,
    pub control_padding_horizontal_sm: f32,
    pub control_padding_horizontal_lg: f32,
}

impl Default for Theme {
    /// The light (`antLight`) palette; `antLight()` returns `Theme::default()`.
    fn default() -> Self {
        Theme {
            color_primary: 0xff1677ff,
            color_primary_hover: 0xff4096ff,
            color_primary_active: 0xff0958d9,
            color_primary_bg: 0xffe6f4ff,

            color_text: 0xe0000000,
            color_text_secondary: 0xa6000000,
            color_text_disabled: 0x40000000,
            color_text_light: 0xffffffff,

            color_bg_base: 0xfff5f5f5,
            color_bg_container: 0xffffffff,
            color_bg_elevated: 0xffffffff,
            color_bg_layout: 0xfff5f5f5,
            color_fill: 0x26000000,
            color_fill_secondary: 0x0f000000,
            color_fill_tertiary: 0x0a000000,
            color_fill_quaternary: 0x05000000,

            color_border: 0xffd9d9d9,
            color_border_secondary: 0xfff0f0f0,
            color_split: 0xfff0f0f0,

            color_success: 0xff52c41a,
            color_warning: 0xfffaad14,
            color_error: 0xffff4d4f,
            color_success_bg: 0xfff6ffed,
            color_success_text: 0xff389e0d,
            color_warning_bg: 0xfffffbe6,
            color_warning_text: 0xffd48806,
            color_error_bg: 0xfffff2f0,
            color_error_text: 0xffcf1322,

            border_radius: 6.0,
            border_radius_sm: 4.0,
            border_radius_lg: 8.0,

            font_size: 14.0,
            font_size_sm: 12.0,
            font_size_lg: 16.0,

            control_height: 32.0,
            control_height_sm: 24.0,
            control_height_lg: 40.0,

            padding: 16.0,
            padding_sm: 8.0,
            padding_lg: 24.0,

            motion_duration_fast: 100,
            motion_duration_mid: 200,
            motion_duration_slow: 300,

            control_padding_horizontal: 15.0,
            control_padding_horizontal_sm: 7.0,
            control_padding_horizontal_lg: 15.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ThemeMode {
    #[default]
    Light,
    Dark,
}

pub fn ant_light() -> Theme {
    Theme::default()
}

/// Dark counterpart of `antLight` (Ant Design dark algorithm values).
pub fn ant_dark() -> Theme {
    Theme {
        color_primary: 0xff1668dc,
        color_primary_hover: 0xff3c89e8,
        color_primary_active: 0xff1554ad,
        color_primary_bg: 0xff111a2c,
        color_text: 0xd9ffffff,
        color_text_secondary: 0xa6ffffff,
        color_text_disabled: 0x40ffffff,
        color_text_light: 0xffffffff,
        color_bg_base: 0xff000000,
        color_bg_container: 0xff141414,
        color_bg_elevated: 0xff1f1f1f,
        color_bg_layout: 0xff000000,
        color_fill: 0x2effffff,
        color_fill_secondary: 0x1fffffff,
        color_fill_tertiary: 0x14ffffff,
        color_fill_quaternary: 0x0affffff,
        color_border: 0xff424242,
        color_border_secondary: 0xff303030,
        color_split: 0x0fffffff,
        color_success_bg: 0xff162312,
        color_success_text: 0xff49aa19,
        color_warning_bg: 0xff2b2111,
        color_warning_text: 0xffd89614,
        color_error_bg: 0xff2c1618,
        color_error_text: 0xffdc4446,
        ..Theme::default()
    }
}

/// Whether `theme` is a dark one (its container background is dark), for
/// components with their own light / dark palettes (Markdown, code).
pub fn is_dark(theme: Theme) -> bool {
    let bg = theme.color_bg_container;
    let luminance = ((bg >> 16) & 255) * 299 + ((bg >> 8) & 255) * 587 + (bg & 255) * 114;
    luminance < 128_000
}

pub fn for_mode(mode: ThemeMode) -> Theme {
    match mode {
        ThemeMode::Light => ant_light(),
        ThemeMode::Dark => ant_dark(),
    }
}

pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> u32 {
    (u32::from(a) << 24) | (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)
}

pub fn alpha(color: u32) -> u8 {
    ((color >> 24) & 0xff) as u8
}

pub fn red(color: u32) -> u8 {
    ((color >> 16) & 0xff) as u8
}

pub fn green(color: u32) -> u8 {
    ((color >> 8) & 0xff) as u8
}

pub fn blue(color: u32) -> u8 {
    (color & 0xff) as u8
}

pub fn lerp(a: u32, b: u32, t: f32) -> u32 {
    let k = t.clamp(0.0, 1.0);
    let ia = i32::from(alpha(a));
    let ib = i32::from(alpha(b));
    let r = f32::from(red(a)) + (f32::from(red(b)) - f32::from(red(a))) * k;
    let g = f32::from(green(a)) + (f32::from(green(b)) - f32::from(green(a))) * k;
    let bl = f32::from(blue(a)) + (f32::from(blue(b)) - f32::from(blue(a))) * k;
    let al = (ia as f32) + ((ib as f32) - (ia as f32)) * k;
    ((al.clamp(0.0, 255.0) as u32) << 24)
        | ((r.clamp(0.0, 255.0) as u32) << 16)
        | ((g.clamp(0.0, 255.0) as u32) << 8)
        | (bl.clamp(0.0, 255.0) as u32)
}

pub fn get(c: u32) -> u32 {
    c
}

pub fn with_alpha(color: u32, a: u8) -> u32 {
    (color & 0x00ff_ffff) | (u32::from(a) << 24)
}

#[cfg(test)]
mod tests {
    #[test]
    fn is_dark() {
        assert!(!super::is_dark(super::ant_light()));
        assert!(super::is_dark(super::ant_dark()));
    }
}
