//! Array, matrix, cases, and alignment environments and their macros.

use super::{Array, Node, ParseError, Parser, Stop, parse_dimen};

impl<'a> Parser<'a> {
    pub(super) fn environment_macro(&mut self, name: &str) -> Result<Option<Node>, ParseError> {
        if name == "substack" {
            // lines centred in script style (amsmath \\substack)
            let body = self.raw_arg()?;
            return Ok(Some(Node::Group(self.sub_parse(&[
                "\\begin{smallmatrix}",
                body,
                "\\end{smallmatrix}",
            ])?)));
        }
        if name == "displaylines" {
            let body = self.raw_arg()?;
            return Ok(Some(Node::Group(self.sub_parse(&[
                "\\begin{gathered}",
                body,
                "\\end{gathered}",
            ])?)));
        }
        Ok(None)
    }

    pub(super) fn environment(&mut self) -> Result<Node, ParseError> {
        let name = self.raw_arg()?;
        let mut left = String::new();
        let mut right = String::new();
        let mut display = false;
        let mut col_sep: f32 = 0.5;
        let mut aligned = false;
        let mut small = false;
        let mut col_spec: &'a str = "";
        let mut default_align: u8 = b'c';
        if name == "matrix" || name == "matrix*" {
            // no delimiters
        } else if name == "pmatrix" {
            left = "(".to_string();
            right = ")".to_string();
        } else if name == "bmatrix" {
            left = "[".to_string();
            right = "]".to_string();
        } else if name == "Bmatrix" {
            left = "{".to_string();
            right = "}".to_string();
        } else if name == "vmatrix" {
            left = "|".to_string();
            right = "|".to_string();
        } else if name == "Vmatrix" {
            left = "‖".to_string();
            right = "‖".to_string();
        } else if name == "smallmatrix" {
            small = true;
            col_sep = 0.2778;
        } else if name == "cases" || name == "dcases" {
            left = "{".to_string();
            col_spec = "ll";
            col_sep = 0.5;
            display = name == "dcases";
        } else if name == "rcases" {
            right = "}".to_string();
            col_spec = "ll";
        } else if name == "aligned"
            || name == "align"
            || name == "align*"
            || name == "split"
            || name == "alignat"
            || name == "alignat*"
            || name == "alignedat"
            || name == "eqnarray"
            || name == "eqnarray*"
        {
            if name.starts_with("alignat") || name == "alignedat" {
                let _ = self.raw_arg()?;
            }
            aligned = true;
            display = true;
            col_sep = 0.0;
        } else if name == "gathered"
            || name == "gather"
            || name == "gather*"
            || name == "equation"
            || name == "equation*"
        {
            display = true;
            col_sep = 0.0;
        } else if name == "array" || name == "darray" {
            col_spec = self.raw_arg()?;
            display = name == "darray";
        } else {
            return Err(ParseError);
        }
        if name.ends_with("matrix*")
            && let Some(a) = self.optional_raw()?
        {
            default_align = if !a.is_empty() { a.as_bytes()[0] } else { b'c' };
        }

        let mut rows: Vec<Vec<Vec<Node>>> = Vec::new();
        let mut row_rules: Vec<usize> = Vec::new();
        let mut gaps: Vec<f32> = Vec::new();
        let mut cells: Vec<Vec<Node>> = Vec::new();
        let mut max_cols: usize = 0;
        loop {
            self.skip_space();
            if self.looking_at_command("hline") {
                self.i += "\\hline".len();
                row_rules.push(rows.len());
                continue;
            }
            let cell = self.parse_list(Stop::EndEnv)?;
            cells.push(cell);
            self.skip_space();
            if self.peek_byte() == b'&' {
                self.i += 1;
                continue;
            }
            if self.looking_at("\\\\") || self.looking_at_command("cr") {
                self.i += if self.looking_at("\\\\") { 2 } else { 3 };
                let mut gap: f32 = 0.0;
                self.skip_space();
                if self.peek_byte() == b'[' {
                    let Some(close) = self.src.as_bytes()[self.i..]
                        .iter()
                        .position(|&b| b == b']')
                        .map(|p| p + self.i)
                    else {
                        return Err(ParseError);
                    };
                    gap = parse_dimen(self.slice(self.i + 1, close));
                    self.i = close + 1;
                }
                max_cols = max_cols.max(cells.len());
                rows.push(std::mem::take(&mut cells));
                gaps.push(gap);
                continue;
            }
            if self.looking_at_command("end") {
                self.i += "\\end".len();
                let end_name = self.raw_arg()?;
                if end_name != name {
                    return Err(ParseError);
                }
                // drop a trailing empty row (`a \\ b \\`)
                let empty_last = cells.len() == 1 && cells[0].is_empty();
                if !empty_last || rows.is_empty() {
                    max_cols = max_cols.max(cells.len());
                    rows.push(std::mem::take(&mut cells));
                    gaps.push(0.0);
                }
                break;
            }
            return Err(ParseError);
        }

        let mut cols: Vec<u8> = Vec::new();
        let mut col_rules: Vec<usize> = Vec::new();
        if !col_spec.is_empty() {
            for c in col_spec.as_bytes() {
                match c {
                    b'l' | b'c' | b'r' => cols.push(*c),
                    b'|' => col_rules.push(cols.len()),
                    _ => {}
                }
            }
        }
        while cols.len() < max_cols {
            let a: u8 = if aligned {
                if cols.len().is_multiple_of(2) {
                    b'r'
                } else {
                    b'l'
                }
            } else if !col_spec.is_empty() {
                b'l'
            } else {
                default_align
            };
            cols.push(a);
        }
        Ok(Node::Array(Array {
            rows,
            cols,
            col_rules,
            row_rules,
            left,
            right,
            display,
            col_sep,
            aligned,
            small,
            row_gaps: gaps,
        }))
    }

    fn optional_raw(&mut self) -> Result<Option<&'a str>, ParseError> {
        self.skip_space();
        if self.peek_byte() != b'[' {
            return Ok(None);
        }
        let Some(close) = self.src.as_bytes()[self.i..]
            .iter()
            .position(|&b| b == b']')
            .map(|p| p + self.i)
        else {
            return Err(ParseError);
        };
        let out = self.slice(self.i + 1, close);
        self.i = close + 1;
        Ok(Some(out))
    }
}
