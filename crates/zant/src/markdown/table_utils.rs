//! Port of `src/markdown/table_utils.zig` — WP-3.2.
//!
//! Port of Streamdown `lib/table/utils.ts`: table → CSV / TSV / Markdown.

pub struct TableData<'a> {
    pub headers: &'a [&'a str],
    pub rows: &'a [&'a [&'a str]],
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Separator {
    #[default]
    Comma,
    Semicolon,
    Tab,
}

impl Separator {
    fn char(self) -> u8 {
        match self {
            Separator::Comma => b',',
            Separator::Semicolon => b';',
            Separator::Tab => b'\t',
        }
    }
}

fn csv_cell(out: &mut String, value: &str, sep: u8) {
    let needs = value
        .bytes()
        .any(|c| c == sep || c == b'"' || c == b'\n' || c == b'\r');
    if !needs {
        out.push_str(value);
        return;
    }
    out.push('"');
    for c in value.chars() {
        if c == '"' {
            out.push('"');
        }
        out.push(c);
    }
    out.push('"');
}

pub fn to_csv(data: &TableData<'_>, separator: Separator) -> String {
    let sep = separator.char();
    let mut out = String::new();
    let mut first_row = true;
    if !data.headers.is_empty() {
        for (i, h) in data.headers.iter().enumerate() {
            if i > 0 {
                out.push(sep as char);
            }
            csv_cell(&mut out, h, sep);
        }
        first_row = false;
    }
    for row in data.rows {
        if !first_row {
            out.push('\n');
        }
        first_row = false;
        for (i, cell) in row.iter().enumerate() {
            if i > 0 {
                out.push(sep as char);
            }
            csv_cell(&mut out, cell, sep);
        }
    }
    out
}

fn tsv_cell(out: &mut String, value: &str) {
    for c in value.chars() {
        match c {
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ => out.push(c),
        }
    }
}

pub fn to_tsv(data: &TableData<'_>) -> String {
    let mut out = String::new();
    let mut first_row = true;
    if !data.headers.is_empty() {
        for (i, h) in data.headers.iter().enumerate() {
            if i > 0 {
                out.push('\t');
            }
            tsv_cell(&mut out, h);
        }
        first_row = false;
    }
    for row in data.rows {
        if !first_row {
            out.push('\n');
        }
        first_row = false;
        for (i, cell) in row.iter().enumerate() {
            if i > 0 {
                out.push('\t');
            }
            tsv_cell(&mut out, cell);
        }
    }
    out
}

pub fn escape_markdown_cell(out: &mut String, cell: &str) {
    for c in cell.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '|' => out.push_str("\\|"),
            '\n' => out.push_str("<br>"),
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
}

pub fn to_markdown(data: &TableData<'_>) -> String {
    let mut out = String::new();
    if data.headers.is_empty() {
        return out;
    }
    out.push_str("| ");
    for (i, h) in data.headers.iter().enumerate() {
        if i > 0 {
            out.push_str(" | ");
        }
        escape_markdown_cell(&mut out, h);
    }
    out.push_str(" |\n| ");
    for i in 0..data.headers.len() {
        if i > 0 {
            out.push_str(" | ");
        }
        out.push_str("---");
    }
    out.push_str(" |");
    for row in data.rows {
        out.push_str("\n| ");
        let n = row.len().max(data.headers.len());
        for i in 0..n {
            if i > 0 {
                out.push_str(" | ");
            }
            if i < row.len() {
                escape_markdown_cell(&mut out, row[i]);
            }
        }
        out.push_str(" |");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_tsv_markdown_match_streamdown() {
        let data = TableData {
            headers: &["Name", "Note"],
            rows: &[&["a,b", "say \"hi\""]],
        };
        let csv = to_csv(&data, Separator::Comma);
        assert_eq!(csv, "Name,Note\n\"a,b\",\"say \"\"hi\"\"\"");
        let md = to_markdown(&TableData {
            headers: &["A|B"],
            rows: &[&["x<y"]],
        });
        assert_eq!(md, "| A\\|B |\n| --- |\n| x&lt;y |");
    }
}
