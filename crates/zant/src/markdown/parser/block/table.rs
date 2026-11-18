//! GFM table headers, alignment, and rows.

use crate::markdown::ast::{self, Block, Span};
use crate::markdown::scan;

use super::Parser;

impl Parser {
    pub(super) fn table(
        &mut self,
        lines: &[&str],
        start: usize,
        out: &mut Vec<Block>,
    ) -> Option<usize> {
        let aligns = scan::table_delimiter(lines[start + 1])?;
        let header_cells = scan::split_table_row(lines[start]);
        if header_cells.len() != aligns.len() {
            return None;
        }
        let cols = aligns.len();
        let mut headers: Vec<Vec<Span>> = Vec::with_capacity(cols);
        let mut raw_headers: Vec<String> = Vec::with_capacity(cols);
        for cell in &header_cells {
            let spans = self.inlines(cell);
            raw_headers.push(if self.structure_only {
                cell.to_string()
            } else {
                ast::plain_text(&spans)
            });
            headers.push(spans);
        }
        let mut rows: Vec<Vec<Vec<Span>>> = Vec::new();
        let mut raw_rows: Vec<Vec<String>> = Vec::new();
        let mut i = start + 2;
        while i < lines.len() {
            let line = lines[i];
            if scan::is_blank(line) || self.interrupts_paragraph(line) {
                break;
            }
            let cells = scan::split_table_row(line);
            let mut row: Vec<Vec<Span>> = Vec::with_capacity(cols);
            let mut raw_row: Vec<String> = Vec::with_capacity(cols);
            for c in 0..cols {
                let cell = if c < cells.len() {
                    cells[c].as_ref()
                } else {
                    ""
                };
                let spans = self.inlines(cell);
                raw_row.push(if self.structure_only {
                    cell.to_string()
                } else {
                    ast::plain_text(&spans)
                });
                row.push(spans);
            }
            rows.push(row);
            raw_rows.push(raw_row);
            i += 1;
        }
        out.push(Block::Table(ast::Table {
            headers,
            rows,
            aligns,
            raw_headers,
            raw_rows,
        }));
        Some(i)
    }
}
