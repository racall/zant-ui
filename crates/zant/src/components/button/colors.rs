//! The antd button palette per variant and state (rest / hover / active),
//! including the `danger` variants. Disabled buttons use one look for every
//! variant, `danger` or not (antd).
#![forbid(unsafe_code)]

use crate::core::node::{ButtonData, ButtonType};
use crate::theme::tokens;
use crate::theme::tokens::Theme;

/// Windows 11 caption buttons: close hover #C42B1C, pressed at 90 %.
const CAPTION_CLOSE_HOVER: u32 = 0xffc4_2b1c;
const CAPTION_CLOSE_PRESSED: u32 = 0xe6c4_2b1c;

const CLEAR: u32 = 0x0000_0000;

/// Fill, border and label colours at rest, on hover and while pressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Colors {
    pub bg: u32,
    pub hover_bg: u32,
    pub active_bg: u32,
    pub border: u32,
    pub hover_border: u32,
    pub active_border: u32,
    pub text: u32,
    pub hover_text: u32,
    pub active_text: u32,
}

impl Colors {
    /// The same fill and border in every state.
    fn fixed(bg: u32, border: u32, text: u32) -> Colors {
        Colors::states([bg; 3], [border; 3], [text; 3])
    }

    /// `[rest, hover, active]` per channel.
    fn states(bg: [u32; 3], border: [u32; 3], text: [u32; 3]) -> Colors {
        Colors {
            bg: bg[0],
            hover_bg: bg[1],
            active_bg: bg[2],
            border: border[0],
            hover_border: border[1],
            active_border: border[2],
            text: text[0],
            hover_text: text[1],
            active_text: text[2],
        }
    }

    /// The colours at `hover_t` / `press_t` (0..1): `(bg, border, text)`.
    pub fn at(&self, hover_t: f32, press_t: f32) -> (u32, u32, u32) {
        let mix = |rest: u32, hover: u32, active: u32| {
            tokens::lerp(tokens::lerp(rest, hover, hover_t), active, press_t)
        };
        (
            mix(self.bg, self.hover_bg, self.active_bg),
            mix(self.border, self.hover_border, self.active_border),
            mix(self.text, self.hover_text, self.active_text),
        )
    }
}

fn is_borderless(button_type: ButtonType) -> bool {
    matches!(
        button_type,
        ButtonType::Text
            | ButtonType::Link
            | ButtonType::Plain
            | ButtonType::Caption
            | ButtonType::CaptionClose
    )
}

/// The palette of `data` (ignores the `text_color` override).
pub fn colors(data: &ButtonData, enabled: bool, theme: Theme) -> Colors {
    if !enabled {
        // antd: disabled text / link buttons have no fill or border
        if is_borderless(data.button_type) {
            let clear = tokens::with_alpha(theme.color_fill_secondary, 0);
            return Colors::fixed(clear, CLEAR, theme.color_text_disabled);
        }
        return Colors::fixed(
            theme.color_fill_secondary,
            theme.color_border,
            theme.color_text_disabled,
        );
    }
    let error = [
        theme.color_error,
        theme.color_error_hover,
        theme.color_error_active,
    ];
    let danger = data.danger;
    match data.button_type {
        ButtonType::Primary if danger => Colors::states(error, error, [theme.color_text_light; 3]),
        ButtonType::Primary => {
            let primary = [
                theme.color_primary,
                theme.color_primary_hover,
                theme.color_primary_active,
            ];
            Colors::states(primary, primary, [theme.color_text_light; 3])
        }
        ButtonType::Default if danger => {
            Colors::states([theme.color_bg_container; 3], error, error)
        }
        ButtonType::Default => Colors::states(
            [theme.color_bg_container; 3],
            [
                theme.color_border,
                theme.color_primary_hover,
                theme.color_primary_active,
            ],
            [theme.color_text; 3],
        ),
        // antd: colorBgTextHover = fillSecondary, colorBgTextActive = fill
        ButtonType::Text => Colors::states(
            [
                tokens::with_alpha(theme.color_fill_secondary, 0),
                theme.color_fill_secondary,
                theme.color_fill,
            ],
            [CLEAR; 3],
            if danger { error } else { [theme.color_text; 3] },
        ),
        ButtonType::Link => Colors::states(
            [CLEAR; 3],
            [CLEAR; 3],
            if danger {
                error
            } else {
                [theme.color_primary; 3]
            },
        ),
        ButtonType::Plain => Colors::fixed(CLEAR, CLEAR, theme.color_text),
        // Windows caption buttons: a faint fill of the text colour
        ButtonType::Caption => Colors::states(
            [
                tokens::with_alpha(theme.color_text, 0),
                tokens::with_alpha(theme.color_text, 0x12),
                tokens::with_alpha(theme.color_text, 0x0c),
            ],
            [CLEAR; 3],
            [theme.color_text; 3],
        ),
        // (paint turns the close glyph white on its red fill, over any
        // `text_color`)
        ButtonType::CaptionClose => Colors::states(
            [
                tokens::with_alpha(CAPTION_CLOSE_HOVER, 0),
                CAPTION_CLOSE_HOVER,
                CAPTION_CLOSE_PRESSED,
            ],
            [CLEAR; 3],
            [theme.color_text; 3],
        ),
    }
}

/// The label colour at rest (measuring uses it, so an idle button never
/// re-shapes its label).
pub fn text_color(data: &ButtonData, enabled: bool, theme: Theme) -> u32 {
    colors(data, enabled, theme).text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::tokens::{ant_dark, ant_light};

    fn data(button_type: ButtonType, danger: bool) -> ButtonData {
        ButtonData {
            button_type,
            danger,
            ..Default::default()
        }
    }

    #[test]
    fn danger_buttons_use_the_error_colours_in_both_themes() {
        for theme in [ant_light(), ant_dark()] {
            let error = [
                theme.color_error,
                theme.color_error_hover,
                theme.color_error_active,
            ];
            let primary = colors(&data(ButtonType::Primary, true), true, theme);
            assert_eq!([primary.bg, primary.hover_bg, primary.active_bg], error);
            assert_eq!(primary.border, theme.color_error);
            assert_eq!(primary.text, theme.color_text_light);
            assert_eq!(primary.active_text, theme.color_text_light);

            let default = colors(&data(ButtonType::Default, true), true, theme);
            assert_eq!(default.bg, theme.color_bg_container);
            assert_eq!(
                [default.border, default.hover_border, default.active_border],
                error
            );
            assert_eq!(
                [default.text, default.hover_text, default.active_text],
                error
            );

            let text = colors(&data(ButtonType::Text, true), true, theme);
            assert_eq!([text.text, text.hover_text, text.active_text], error);
            // the text button keeps its neutral hover / press fill
            assert_eq!(text.hover_bg, theme.color_fill_secondary);
            assert_eq!(text.active_bg, theme.color_fill);

            let link = colors(&data(ButtonType::Link, true), true, theme);
            assert_eq!([link.text, link.hover_text, link.active_text], error);
            assert_eq!(link.hover_bg, CLEAR);
        }
    }

    #[test]
    fn disabled_danger_buttons_look_like_any_disabled_button() {
        for theme in [ant_light(), ant_dark()] {
            for variant in [
                ButtonType::Primary,
                ButtonType::Default,
                ButtonType::Text,
                ButtonType::Link,
            ] {
                assert_eq!(
                    colors(&data(variant, true), false, theme),
                    colors(&data(variant, false), false, theme)
                );
            }
            let primary = colors(&data(ButtonType::Primary, true), false, theme);
            assert_eq!(primary.bg, theme.color_fill_secondary);
            assert_eq!(primary.text, theme.color_text_disabled);
        }
    }

    #[test]
    fn plain_variants_keep_their_colours() {
        let theme = ant_light();
        let default = colors(&data(ButtonType::Default, false), true, theme);
        assert_eq!(default.border, theme.color_border);
        assert_eq!(default.hover_border, theme.color_primary_hover);
        assert_eq!(default.text, theme.color_text);
        assert_eq!(default.hover_text, theme.color_text);
        let primary = colors(&data(ButtonType::Primary, false), true, theme);
        assert_eq!(primary.hover_bg, theme.color_primary_hover);
        // the state mix: halfway hovered, then fully pressed
        let (bg, _, _) = primary.at(1.0, 1.0);
        assert_eq!(bg, theme.color_primary_active);
        let (bg, _, _) = primary.at(0.0, 0.0);
        assert_eq!(bg, theme.color_primary);
    }
}
