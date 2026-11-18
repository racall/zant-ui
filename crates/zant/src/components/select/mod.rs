//! The closed antd select control (field, selected label, chevron) plus the
//! data helpers the runtime needs for its overlay: the popup geometry / paint
//! and the tooltip layer.
//!
//! The popup and tooltip *layers* are owned by the runtime. This module only
//! provides the component-side helpers: [`popup_rect`], [`option_at`],
//! [`paint_popup`], [`animate`] and the [`tooltip`] helpers. The runtime owns
//! the open/popup `NodeId`, the hovered option pointer handling, keyboard
//! navigation and the tooltip's delay / layout / transition state.
#![forbid(unsafe_code)]

mod field;
mod popup;
pub mod tooltip;

#[cfg(test)]
mod tests;

use crate::core::node::SelectData;
use crate::motion::motion;
use crate::theme::tokens::Theme;

pub use field::{
    FIELD_HEIGHT, PADDING_X, control_height, ensure_layout, font_size, horizontal_padding,
    measure_height, measure_width, paint, selected_label,
};
pub use popup::{POPUP_PADDING, ROW_HEIGHT, option_at, paint_popup, popup_rect};

/// Border / ring / popup transitions (antd 200ms). Returns true while
/// another frame is needed.
pub fn animate(data: &mut SelectData, now: i64, theme: Theme) -> bool {
    let mid = motion::Transition {
        duration_ms: theme.motion_duration_mid as u16,
        easing: crate::motion::easing::Easing::EaseInOut,
        delay_ms: 0,
    };
    data.hover
        .retarget(if data.hovered { 1.0 } else { 0.0 }, now, mid);
    data.focus_anim
        .retarget(if data.focused || data.open { 1.0 } else { 0.0 }, now, mid);
    data.open_anim.retarget(
        if data.open { 1.0 } else { 0.0 },
        now,
        if data.open {
            motion::presence_enter(motion::Presence::SlideUp)
        } else {
            motion::presence_exit(motion::Presence::SlideUp)
        },
    );
    data.hover_t = data.hover.value(now);
    data.focus_t = data.focus_anim.value(now);
    data.open_t = data.open_anim.value(now);
    !(data.hover.done(now) && data.focus_anim.done(now) && data.open_anim.done(now))
}
