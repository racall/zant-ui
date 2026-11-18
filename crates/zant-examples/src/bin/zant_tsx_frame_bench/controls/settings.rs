//! `settings.tsx` written by hand in Rust: the control group of the
//! `settings-click` scenario.
//!
//! Same tree as `tsx/corpus/settings.tsx`: one scroll view, five sections of
//! eight rows, each row a label/subtitle column plus an independent
//! `ui::switch`, and the `Settings · N changes` title. The TSX page spells the
//! 40 rows out; this control builds them from the same label table (the nodes
//! are identical).
#![forbid(unsafe_code)]

use zant::Str;
use zant::flex;
use zant::renderer::text;
use zant::ui::{self, App, BoxOptions, TextOptions, ToggleOptions, ViewOptions};

/// The sections and row labels of `settings.tsx`, in order.
const SECTIONS: &[(&str, &[&str])] = &[
    (
        "general",
        &[
            "Launch at login",
            "Show in menu bar",
            "Check for updates",
            "Send usage data",
            "Open last project",
            "Restore windows",
            "Confirm on quit",
            "Show welcome page",
        ],
    ),
    (
        "editor",
        &[
            "Line numbers",
            "Word wrap",
            "Auto save",
            "Format on save",
            "Show whitespace",
            "Highlight line",
            "Bracket pairs",
            "Smooth caret",
        ],
    ),
    (
        "appearance",
        &[
            "Dark mode",
            "Compact density",
            "Animations",
            "Large icons",
            "Status bar",
            "Tab bar",
            "Breadcrumbs",
            "Minimap",
        ],
    ),
    (
        "sync",
        &[
            "Sync settings",
            "Sync extensions",
            "Sync keybindings",
            "Sync snippets",
            "Sync on metered",
            "Conflict prompts",
            "Backups",
            "Sync history",
        ],
    ),
    (
        "privacy",
        &[
            "Crash reports",
            "Telemetry",
            "Spell check online",
            "Link previews",
            "Remote images",
            "Location hints",
            "Read receipts",
            "Typing status",
        ],
    ),
];

/// The settings page state.
pub struct Settings {
    changes: u32,
    /// One independent `useState(false)` per switch.
    flags: [bool; 40],
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            changes: 0,
            flags: [false; 40],
        }
    }
}

impl Settings {
    pub fn new() -> Settings {
        Settings::default()
    }

    fn set(&mut self, index: usize, on: bool) {
        self.flags[index] = on;
        self.changes += 1;
    }

    fn row(&self, index: usize, label: &'static str) -> ui::Node {
        let on = self.flags[index];
        let option = index + 1;
        ui::row(BoxOptions {
            key: Some(Str::copy(&format!("row-{index}"))),
            gap: 16.0,
            padding: zant::core::geometry::Edges {
                top: 8.0,
                bottom: 8.0,
                ..Default::default()
            },
            align_items: flex::Align::Center,
            children: vec![
                ui::view(ViewOptions {
                    grow: 1.0,
                    shrink: 1.0,
                    basis: Some(0.0),
                    gap: 2.0,
                    children: vec![
                        ui::text(label, TextOptions::default()),
                        ui::text(
                            format!("{} · option {option} of 40", if on { "On" } else { "Off" }),
                            TextOptions {
                                size: 12.0,
                                ..TextOptions::default()
                            },
                        ),
                    ],
                    ..ViewOptions::default()
                }),
                ui::r#switch(ToggleOptions {
                    key: Some(Str::copy(&format!("sw-{index}"))),
                    checked: on,
                    on_change: Some(ui::bool_action(move |state: &mut Settings, on: bool| {
                        state.set(index, on)
                    })),
                    ..ToggleOptions::default()
                }),
            ],
            ..BoxOptions::default()
        })
    }
}

impl App for Settings {
    fn render(&mut self) -> ui::Node {
        let mut children = Vec::with_capacity(SECTIONS.len() + 1);
        children.push(ui::text(
            format!("Settings · {} changes", self.changes),
            TextOptions {
                key: Some(Str::Static("changes")),
                size: 20.0,
                weight: text::Weight::Semibold,
                ..TextOptions::default()
            },
        ));
        let mut index = 0usize;
        for (section, labels) in SECTIONS {
            let mut rows = Vec::with_capacity(labels.len() + 1);
            rows.push(ui::text(
                section_label(section),
                TextOptions {
                    size: 16.0,
                    weight: text::Weight::Medium,
                    ..TextOptions::default()
                },
            ));
            for label in *labels {
                rows.push(self.row(index, label));
                index += 1;
            }
            children.push(ui::view(ViewOptions {
                key: Some(Str::copy(section)),
                gap: 4.0,
                children: rows,
                ..ViewOptions::default()
            }));
        }
        debug_assert_eq!(index, 40);
        ui::view(ViewOptions {
            padding: ui::inset(24.0),
            gap: 20.0,
            scroll: true,
            height_percent: Some(100.0),
            children,
            ..ViewOptions::default()
        })
    }
}

/// The antd-style section title of `settings.tsx` (`General`, `Editor`, …).
fn section_label(section: &str) -> &'static str {
    match section {
        "general" => "General",
        "editor" => "Editor",
        "appearance" => "Appearance",
        "sync" => "Sync",
        _ => "Privacy",
    }
}
