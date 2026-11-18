//! Table (Streamdown `table/*`): bordered grid with a header row and
//! copy (Markdown / CSV) and download (CSV or Markdown) controls.

use crate::Str;
use crate::core::geometry::Edges;
use crate::flex;
use crate::markdown::ast;
use crate::markdown::render::actions::{Copy, Download};
use crate::markdown::render::context::Ctx;
use crate::markdown::render::controls;
use crate::markdown::render::spans::{self, Base};
use crate::markdown::table_utils::{self, TableData};
use crate::renderer::text;
use crate::ui;
use crate::ui::options::{BoxOptions, DividerOptions};

fn cell_align(align: ast::Align) -> flex::Align {
    match align {
        ast::Align::Center => flex::Align::Center,
        ast::Align::Right => flex::Align::End,
        _ => flex::Align::Start,
    }
}

fn row_node(
    ctx: &Ctx,
    cells: &[Vec<ast::Span>],
    aligns: &[ast::Align],
    header: bool,
    key: &str,
) -> ui::Node {
    let theme = ctx.options.theme;
    let mut children: Vec<ui::Node> = Vec::with_capacity(cells.len());
    for (c, cell) in cells.iter().enumerate() {
        let content = spans::rich_node(
            ctx,
            cell,
            Base {
                size: ctx.size * 0.875,
                color: ctx.fg,
                weight: if header {
                    text::Weight::Semibold
                } else {
                    text::Weight::Regular
                },
                family: ctx.family(),
                ..Base::default()
            },
            false,
        );
        children.push(ui::column(BoxOptions {
            grow: 1.0,
            shrink: 1.0,
            basis: Some(0.0),
            padding: Edges {
                left: 12.0,
                right: 12.0,
                top: 8.0,
                bottom: 8.0,
            },
            align_items: cell_align(aligns.get(c).copied().unwrap_or(ast::Align::None)),
            children: vec![content],
            ..Default::default()
        }));
    }
    ui::row(BoxOptions {
        key: Some(Str::copy(key)),
        width_percent: Some(100.0),
        align_items: flex::Align::Stretch,
        background: if header {
            Some(theme.table_header_bg)
        } else {
            None
        },
        children,
        ..Default::default()
    })
}

pub fn render(ctx: &Ctx, table: &ast::Table) -> ui::Node {
    let theme = ctx.options.theme;
    let mut rows: Vec<ui::Node> = Vec::with_capacity(table.rows.len() * 2 + 1);
    rows.push(row_node(
        ctx,
        &table.headers,
        &table.aligns,
        true,
        &ctx.sub_key("h"),
    ));
    for (r, row) in table.rows.iter().enumerate() {
        rows.push(ui::divider(DividerOptions {
            color: Some(theme.table_border),
            ..Default::default()
        }));
        rows.push(row_node(
            ctx,
            row,
            &table.aligns,
            false,
            &ctx.sub_key(&format!("r{r}")),
        ));
    }
    let grid = ui::column(BoxOptions {
        key: Some(Str::copy(&ctx.sub_key("grid"))),
        width_percent: Some(100.0),
        border_color: Some(theme.table_border),
        border_width: 1.0,
        radius: 8.0,
        clip: true,
        align_items: flex::Align::Stretch,
        children: rows,
        ..Default::default()
    });
    let tc = ctx.options.controls.table;
    if !tc.copy && !tc.download {
        return grid;
    }

    let headers: Vec<&str> = table.raw_headers.iter().map(String::as_str).collect();
    let raw_rows: Vec<Vec<&str>> = table
        .raw_rows
        .iter()
        .map(|row| row.iter().map(String::as_str).collect())
        .collect();
    let row_refs: Vec<&[&str]> = raw_rows.iter().map(Vec::as_slice).collect();
    let data = TableData {
        headers: &headers,
        rows: &row_refs,
    };
    let md = table_utils::to_markdown(&data);
    let csv = table_utils::to_csv(&data, table_utils::Separator::Comma);
    let busy = ctx.options.is_animating && ctx.caret;
    let mut buttons: Vec<ui::Node> = Vec::with_capacity(3);
    if tc.copy {
        buttons.push(controls::button(
            ctx,
            &ctx.sub_key("copy-md"),
            &ctx.options.labels.copy_markdown,
            Some(Copy::action(&md)),
            !busy,
        ));
        buttons.push(controls::button(
            ctx,
            &ctx.sub_key("copy-csv"),
            &ctx.options.labels.copy_csv,
            Some(Copy::action(&csv)),
            !busy,
        ));
    }
    if tc.download {
        buttons.push(controls::button(
            ctx,
            &ctx.sub_key("download"),
            &ctx.options.labels.download,
            Some(Download::action(Download {
                filename: "table.csv".to_owned(),
                extensions: "csv,md".to_owned(),
                text: csv,
                alt_extension: ".md".to_owned(),
                alt_text: md,
            })),
            !busy,
        ));
    }
    ui::column(BoxOptions {
        key: Some(Str::copy(&ctx.key)),
        width_percent: Some(100.0),
        gap: 4.0,
        align_items: flex::Align::Stretch,
        children: vec![
            ui::row(BoxOptions {
                width_percent: Some(100.0),
                justify: flex::Justify::End,
                gap: controls::GAP,
                children: buttons,
                ..Default::default()
            }),
            grid,
        ],
        ..Default::default()
    })
}
