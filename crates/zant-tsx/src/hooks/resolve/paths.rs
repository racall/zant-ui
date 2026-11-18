//! Import paths: relative sources rewritten for the file a hook is inlined into, POSIX normalization, assets.

/// `source` as the caller's file writes it: relative paths are relative to
/// the importing file's directory.
pub(super) fn source_for(from_path: &[u8], caller_path: &[u8], source: &[u8]) -> Vec<u8> {
    if !source.starts_with(b"./") && !source.starts_with(b"../") {
        return source.to_vec();
    }
    let target = normalized(&asset_path(from_path, source));
    let dir = dirname(caller_path)
        .map(normalized)
        .unwrap_or_else(|| normalized(b"."));
    relative(&dir, &target)
}

pub(super) fn normalized(path: &[u8]) -> Vec<u8> {
    let slashes: Vec<u8> = path
        .iter()
        .map(|&b| if b == b'\\' { b'/' } else { b })
        .collect();
    resolve_posix(&slashes)
}

/// A path with `.` and `..` resolved lexically (POSIX separators only).
pub(super) fn resolve_posix(path: &[u8]) -> Vec<u8> {
    let is_abs = path.starts_with(b"/");
    let mut negative: usize = 0;
    let mut parts: Vec<&[u8]> = Vec::new();
    for component in path.split(|&b| b == b'/') {
        if component.is_empty() || component == b"." {
            continue;
        }
        if component == b".." {
            if parts.is_empty() {
                if !is_abs {
                    negative += 1;
                }
            } else {
                parts.pop();
            }
        } else {
            parts.push(component);
        }
    }
    let mut joined: Vec<u8> = Vec::new();
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            joined.push(b'/');
        }
        joined.extend_from_slice(part);
    }
    if is_abs {
        let mut out = vec![b'/'];
        out.extend_from_slice(&joined);
        out
    } else if negative > 0 {
        let mut out = Vec::new();
        for _ in 0..negative {
            out.extend_from_slice(b"../");
        }
        if joined.is_empty() {
            // "../" n times minus the trailing slash
            out.truncate(out.len() - 1);
        } else {
            out.extend_from_slice(&joined);
        }
        out
    } else if joined.is_empty() {
        b".".to_vec()
    } else {
        joined
    }
}

/// `dir` and `target` are both relative to the same directory (or both
/// absolute).
pub fn relative(dir: &[u8], target: &[u8]) -> Vec<u8> {
    let dir_parts: Vec<&[u8]> = dir
        .split(|&b| b == b'/')
        .filter(|p| !p.is_empty() && *p != b".")
        .collect();
    let target_parts: Vec<&[u8]> = target
        .split(|&b| b == b'/')
        .filter(|p| !p.is_empty() && *p != b".")
        .collect();
    let mut common: usize = 0;
    while common < dir_parts.len()
        && common + 1 < target_parts.len()
        && dir_parts[common] == target_parts[common]
    {
        common += 1;
    }
    let mut out: Vec<u8> = Vec::new();
    if common == dir_parts.len() {
        out.extend_from_slice(b"./");
    }
    for _ in &dir_parts[common..] {
        out.extend_from_slice(b"../");
    }
    for (i, p) in target_parts[common..].iter().enumerate() {
        if i > 0 {
            out.push(b'/');
        }
        out.extend_from_slice(p);
    }
    out
}

pub(super) fn basename(path: &[u8]) -> &[u8] {
    for part in path.rsplit(|&b| b == b'/') {
        if !part.is_empty() {
            return part;
        }
    }
    b""
}

pub(super) fn trim_end_slashes(path: &[u8]) -> &[u8] {
    let mut end = path.len();
    while end > 0 && path[end - 1] == b'/' {
        end -= 1;
    }
    &path[..end]
}

pub(super) fn dirname(path: &[u8]) -> Option<&[u8]> {
    let trimmed = trim_end_slashes(path);
    if trimmed.is_empty() {
        return None;
    }
    match trimmed.iter().rposition(|&b| b == b'/') {
        None => None,
        Some(0) => Some(b"/"),
        Some(i) => Some(&trimmed[..i]),
    }
}

pub(super) fn extension(path: &[u8]) -> &[u8] {
    let filename = basename(path);
    match filename.iter().rposition(|&b| b == b'.') {
        None | Some(0) => b"",
        Some(i) => &filename[i..],
    }
}

pub(super) fn join_paths(dir: &[u8], source: &[u8]) -> Vec<u8> {
    if source.starts_with(b"/") {
        return source.to_vec();
    }
    if dir.is_empty() {
        return source.to_vec();
    }
    let mut out = dir.to_vec();
    if !out.ends_with(b"/") {
        out.push(b'/');
    }
    out.extend_from_slice(source);
    out
}

/// A relative import with an extension other than a source file's names a
/// file whose bytes are embedded.
pub(super) fn is_asset(source: &[u8]) -> bool {
    if !source.starts_with(b"./") && !source.starts_with(b"../") {
        return false;
    }
    let ext = extension(source);
    if ext.is_empty() {
        return false;
    }
    ![
        b".tsx".as_slice(),
        b".ts".as_slice(),
        b".rs".as_slice(),
        b".jsx".as_slice(),
        b".js".as_slice(),
    ]
    .contains(&ext)
}

/// The asset's path on disk, relative to the importing file.
pub(super) fn asset_path(importer: &[u8], source: &[u8]) -> Vec<u8> {
    let dir = dirname(importer).unwrap_or(b".");
    join_paths(dir, source)
}
