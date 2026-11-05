//! URL hardening for links and images (Streamdown's rehype-harden).
//!
//! The parser keeps every URL; the renderer asks `check` before making a span
//! clickable or loading an image. Blocked links render as plain text.
const std = @import("std");

pub const Policy = struct {
    /// Allowed link prefixes. Empty = any URL with a safe scheme.
    allowed_link_prefixes: []const []const u8 = &.{},
    /// Allowed image prefixes. Empty = any URL with a safe scheme.
    allowed_image_prefixes: []const []const u8 = &.{},
    /// Allow `data:image/...` sources for images.
    allow_data_images: bool = true,
    /// Allow local paths (relative or absolute) for images.
    allow_local_images: bool = true,
    /// Autolink schemes to ignore entirely (Streamdown's
    /// `disableAutolinkProtocols`, e.g. "mailto").
    disabled_autolink_protocols: []const []const u8 = &.{},
};

pub const Kind = enum { link, image };

const dangerous = [_][]const u8{ "javascript:", "vbscript:", "file:", "data:" };

fn hasScheme(url: []const u8) ?[]const u8 {
    for (url, 0..) |c, i| {
        if (c == ':') return if (i > 0) url[0 .. i + 1] else null;
        if (!(std.ascii.isAlphanumeric(c) or c == '+' or c == '-' or c == '.')) return null;
    }
    return null;
}

fn startsWithIgnoreCase(text: []const u8, prefix: []const u8) bool {
    return text.len >= prefix.len and std.ascii.eqlIgnoreCase(text[0..prefix.len], prefix);
}

/// Returns true when `url` may be used as a link target / image source.
pub fn check(policy: Policy, kind: Kind, raw: []const u8) bool {
    const url = std.mem.trim(u8, raw, " \t\r\n");
    if (url.len == 0) return false;
    if (std.mem.eql(u8, url, "#incomplete")) return false;
    const scheme = hasScheme(url);
    if (scheme) |s| {
        for (dangerous) |bad| {
            if (std.ascii.eqlIgnoreCase(s, bad)) {
                if (kind == .image and policy.allow_data_images and startsWithIgnoreCase(url, "data:image/")) break;
                return false;
            }
        }
    } else if (kind == .image and !policy.allow_local_images) {
        return false;
    }
    const prefixes = if (kind == .link) policy.allowed_link_prefixes else policy.allowed_image_prefixes;
    if (prefixes.len == 0) return true;
    // relative URLs and fragments are always allowed for links
    if (scheme == null and kind == .link) return true;
    for (prefixes) |prefix| {
        if (startsWithIgnoreCase(url, prefix)) return true;
    }
    return false;
}

pub fn autolinkAllowed(policy: Policy, url: []const u8) bool {
    const scheme = hasScheme(url) orelse return true;
    for (policy.disabled_autolink_protocols) |protocol| {
        const p = std.mem.trimEnd(u8, protocol, ":");
        if (scheme.len == p.len + 1 and std.ascii.eqlIgnoreCase(scheme[0..p.len], p)) return false;
    }
    return true;
}

/// True for URLs that leave the app (shown in the link-safety dialog).
pub fn isExternal(url: []const u8) bool {
    return hasScheme(url) != null or std.mem.startsWith(u8, url, "www.");
}

test "dangerous schemes are blocked" {
    const p: Policy = .{};
    try std.testing.expect(!check(p, .link, "javascript:alert(1)"));
    try std.testing.expect(!check(p, .link, " JavaScript:alert(1)"));
    try std.testing.expect(check(p, .link, "https://example.com"));
    try std.testing.expect(check(p, .image, "data:image/png;base64,AAAA"));
    try std.testing.expect(!check(p, .link, "data:text/html,<b>"));
    try std.testing.expect(check(p, .image, "images/a.png"));
}

test "prefix allow-list" {
    const p: Policy = .{ .allowed_link_prefixes = &.{"https://docs.example.com"} };
    try std.testing.expect(check(p, .link, "https://docs.example.com/a"));
    try std.testing.expect(!check(p, .link, "https://evil.example.com"));
    try std.testing.expect(check(p, .link, "#section"));
}
