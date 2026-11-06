//! Platform-aware keyboard shortcuts.
//!
//! Apps describe a shortcut once and get the platform's modifier and label:
//!
//!     if (zant.keys.primary(key.modifiers) and ...)    // ⌘ on macOS, Ctrl on Windows
//!     .keys = zant.keys.label("mod+shift+s"),          // "⇧⌘S" / "Ctrl+Shift+S"
//!     .keys = zant.keys.pick("alt+mod+f", "mod+h"),    // macOS vs Windows chord
//!
//! Spec syntax: chords separated by spaces, keys in a chord joined by "+".
//! Modifiers: `mod` (the primary modifier), `ctrl`, `alt`, `shift`. Keys:
//! letters, digits, punctuation, `enter`, `esc`, `tab`, `space`, `backspace`,
//! `delete`, `home`, `end`, `left`, `right`, `up`, `down`, `f1`..`f12`.
const std = @import("std");
const builtin = @import("builtin");
const events = @import("events.zig");

pub const is_macos = builtin.os.tag == .macos;

/// The platform's command modifier is held: ⌘ on macOS, Ctrl elsewhere.
pub fn primary(m: events.Modifiers) bool {
    return if (is_macos) m.meta else m.control;
}

/// `mac` on macOS, `other` elsewhere (comptime spec strings or any value).
pub inline fn pick(comptime mac: anytype, comptime other: anytype) @TypeOf(if (is_macos) mac else other) {
    return if (is_macos) mac else other;
}

/// Display label of a shortcut spec for the current platform.
/// Comptime-known, so it concatenates: `"保存 " ++ keys.label("mod+s")`.
pub inline fn label(comptime spec: []const u8) []const u8 {
    return comptime labelFor(spec, is_macos);
}

fn keyName(comptime key: []const u8, comptime mac: bool) []const u8 {
    const eql = std.mem.eql;
    if (eql(u8, key, "enter")) return if (mac) "↵" else "Enter";
    if (eql(u8, key, "esc")) return "Esc";
    if (eql(u8, key, "tab")) return if (mac) "⇥" else "Tab";
    if (eql(u8, key, "space")) return "Space";
    if (eql(u8, key, "backspace")) return if (mac) "⌫" else "Backspace";
    if (eql(u8, key, "delete")) return if (mac) "⌦" else "Delete";
    if (eql(u8, key, "home")) return if (mac) "↖" else "Home";
    if (eql(u8, key, "end")) return if (mac) "↘" else "End";
    if (eql(u8, key, "left")) return "←";
    if (eql(u8, key, "right")) return "→";
    if (eql(u8, key, "up")) return "↑";
    if (eql(u8, key, "down")) return "↓";
    if (key.len == 1 and key[0] >= 'a' and key[0] <= 'z') return &[_]u8{key[0] - 32};
    if (key.len >= 2 and key[0] == 'f' and key[1] >= '0' and key[1] <= '9') return "F" ++ key[1..];
    return key;
}

fn chordLabel(comptime chord: []const u8, comptime mac: bool) []const u8 {
    comptime {
        @setEvalBranchQuota(20_000);
        var has_mod = false;
        var has_ctrl = false;
        var has_alt = false;
        var has_shift = false;
        var key: []const u8 = "";
        var it = std.mem.splitScalar(u8, chord, '+');
        while (it.next()) |part| {
            // "mod++" (the plus key): an empty part after a '+' is the key
            if (part.len == 0) {
                key = "+";
            } else if (std.mem.eql(u8, part, "mod")) {
                has_mod = true;
            } else if (std.mem.eql(u8, part, "ctrl")) {
                has_ctrl = true;
            } else if (std.mem.eql(u8, part, "alt")) {
                has_alt = true;
            } else if (std.mem.eql(u8, part, "shift")) {
                has_shift = true;
            } else {
                key = part;
            }
        }
        const name = keyName(key, mac);
        if (mac) {
            // Apple order: ⌃ ⌥ ⇧ ⌘, no separators
            return (if (has_ctrl) "⌃" else "") ++ (if (has_alt) "⌥" else "") ++
                (if (has_shift) "⇧" else "") ++ (if (has_mod) "⌘" else "") ++ name;
        }
        // Windows order: Ctrl+Alt+Shift+Key
        return (if (has_mod or has_ctrl) "Ctrl+" else "") ++ (if (has_alt) "Alt+" else "") ++
            (if (has_shift) "Shift+" else "") ++ name;
    }
}

fn labelFor(comptime spec: []const u8, comptime mac: bool) []const u8 {
    comptime {
        @setEvalBranchQuota(20_000);
        var out: []const u8 = "";
        var it = std.mem.tokenizeScalar(u8, spec, ' ');
        while (it.next()) |chord| {
            if (out.len > 0) out = out ++ "  ";
            out = out ++ chordLabel(chord, mac);
        }
        return out;
    }
}

test "shortcut labels follow each platform's convention" {
    try std.testing.expectEqualStrings("⇧⌘S", comptime labelFor("mod+shift+s", true));
    try std.testing.expectEqualStrings("Ctrl+Shift+S", comptime labelFor("mod+shift+s", false));
    try std.testing.expectEqualStrings("⌥⌘F", comptime labelFor("alt+mod+f", true));
    try std.testing.expectEqualStrings("Ctrl+Alt+F", comptime labelFor("alt+mod+f", false));
    try std.testing.expectEqualStrings("⌘+  ⌘-  ⌘0", comptime labelFor("mod++ mod+- mod+0", true));
    try std.testing.expectEqualStrings("Ctrl++  Ctrl+-  Ctrl+0", comptime labelFor("mod++ mod+- mod+0", false));
    try std.testing.expectEqualStrings("↵  ⇧↵", comptime labelFor("enter shift+enter", true));
    try std.testing.expectEqualStrings("Enter  Shift+Enter", comptime labelFor("enter shift+enter", false));
    try std.testing.expectEqualStrings("⌘\\", comptime labelFor("mod+\\", true));
    try std.testing.expectEqualStrings("Esc", comptime labelFor("esc", false));
}

test "primary modifier" {
    const m: events.Modifiers = if (is_macos) .{ .meta = true } else .{ .control = true };
    try std.testing.expect(primary(m));
    try std.testing.expect(!primary(.{ .shift = true }));
}
