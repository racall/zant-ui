use super::*;

/// GetSubstitution, retaining UTF-16 slices until the final output is decoded.
pub(super) fn expand(m: &RegExpMatch, replacement: &str) -> Vec<u16> {
    let units = &m.0.input.units;
    let captures = m.0.found.captures.len();
    let bytes = replacement.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' && i + 1 < bytes.len() {
            let mut consumed = 2;
            let range = match bytes[i + 1] {
                b'$' => {
                    out.push(u16::from(b'$'));
                    i += 2;
                    continue;
                }
                b'&' => Some(m.start()..m.end()),
                b'`' => Some(0..m.start()),
                b'\'' => Some(m.end()..units.len()),
                b'<' if m.has_groups() => {
                    if let Some(end) = replacement[i + 2..].find('>') {
                        let name = &replacement[i + 2..i + 2 + end];
                        if let Some(r) = m.named_range(name) {
                            out.extend_from_slice(&units[r]);
                        }
                        i += end + 3;
                        continue;
                    }
                    None
                }
                b'0'..=b'9' => {
                    let mut n = usize::from(bytes[i + 1] - b'0');
                    if let Some(d) = bytes.get(i + 2).filter(|d| d.is_ascii_digit()) {
                        let nn = n * 10 + usize::from(d - b'0');
                        if nn > 0 && nn <= captures {
                            n = nn;
                            consumed = 3;
                        }
                    }
                    if n > 0 && n <= captures {
                        if let Some(r) = m.0.found.group(n) {
                            out.extend_from_slice(&units[r]);
                        }
                        i += consumed;
                        continue;
                    }
                    None
                }
                _ => None,
            };
            if let Some(r) = range {
                out.extend_from_slice(&units[r]);
                i += consumed;
                continue;
            }
        }
        let c = replacement[i..].chars().next().unwrap();
        let mut buf = [0; 2];
        out.extend_from_slice(c.encode_utf16(&mut buf));
        i += c.len_utf8();
    }
    out
}
