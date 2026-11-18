//! Table rows against what a browser makes of the same HTML (issue #47): the widths and heights
//! below are Chrome's, measured on the HTML each test quotes.

use mime::layout::{Block, Cell, VAlign, Width, html_layout};

use super::*;

/// The first row of the first table in `html`, as the layout reads it.
fn first_row(html: &str) -> Vec<Cell> {
    let blocks = html_layout(html);
    let Some(Block::Table(t)) = blocks.first() else { panic!("a table: {blocks:?}") };
    t.rows.first().cloned().unwrap_or_default()
}

fn close(got: &[f32], want: &[f32]) -> bool {
    got.len() == want.len() && got.iter().zip(want).all(|(g, w)| (g - w).abs() < 0.5)
}

/// `<table width=600 cellspacing=4 cellpadding=6>` of three 33% cells: Chrome makes each 194.67
/// wide, the 1% left over shared out; it went to the last cell (192.7, 192.7, 198.6).
#[test]
fn percentage_cells_share_the_row_as_a_browser_does() {
    let row = first_row(
        r#"<table width="600" cellspacing="4" cellpadding="6"><tr><td width="33%">a</td><td width="33%">b</td><td width="33%">c</td></tr></table>"#,
    );
    // The row inside the table's own spacing (600 less 4 on each side).
    let (w, stack) = cell_widths(&row, &column_widths(std::slice::from_ref(&row)), &[], 592.0, 4.0);
    assert!(!stack);
    assert!(close(&w, &[194.67, 194.67, 194.67]), "{w:?}");
    // A share, a cell that asks for nothing and one of 150 pixels: 150, 300, 150 in Chrome.
    let row =
        first_row(r#"<table width="600" cellspacing="0" cellpadding="0"><tr><td width="25%">a</td><td>b</td><td width="150">c</td></tr></table>"#);
    let (w, _) = cell_widths(&row, &column_widths(std::slice::from_ref(&row)), &[], 600.0, 0.0);
    assert!(close(&w, &[150.0, 300.0, 150.0]), "{w:?}");
}

/// `<td height=100 valign=top>` beside a cell of two lines (middle, as HTML has it unsaid) and
/// one `valign=bottom`, lines 18 pixels tall: the row is 100 tall, and their text starts 0, 32
/// and 82 down, as in Chrome. A row of `height=80` with `cellpadding=10`: text 31 down.
#[test]
fn a_rows_height_is_a_minimum_and_its_cells_line_up_as_they_say() {
    let row = first_row(
        r#"<table width="600" cellspacing="0" cellpadding="0"><tr><td height="100" valign="top">top</td><td>middle<br>by default</td><td valign="bottom">bottom</td></tr></table>"#,
    );
    assert_eq!(
        row.iter().map(|c| (c.valign, c.height)).collect::<Vec<_>>(),
        [(VAlign::Top, Some(100)), (VAlign::Middle, None), (VAlign::Bottom, None)]
    );
    let asked: Vec<f32> = row.iter().map(|c| c.height.map_or(0.0, f32::from)).collect();
    let (h, down) = row_heights(&[18.0, 36.0, 18.0], &asked, &[VAlign::Top, VAlign::Middle, VAlign::Bottom]);
    assert_eq!(h, 100.0);
    assert!(close(&down, &[0.0, 32.0, 82.0]), "{down:?}");
    // Taller content than asked: the row grows to it.
    let (h, _) = row_heights(&[18.0, 140.0, 18.0], &asked, &[VAlign::Top; 3]);
    assert_eq!(h, 140.0);
    let row = first_row(r#"<table width="600" cellspacing="0" cellpadding="10"><tr height="80"><td>a</td><td valign="middle">b</td></tr></table>"#);
    assert!(row.iter().all(|c| c.height == Some(80) && c.valign == VAlign::Middle), "{row:?}");
    // Content 18 tall and padding 10 above and below; the content starts 10 + 21 = 31 down.
    let (h, down) = row_heights(&[38.0, 38.0], &[80.0, 80.0], &[VAlign::Middle; 2]);
    assert_eq!(h, 80.0);
    assert!(close(&down, &[21.0, 21.0]), "{down:?}");
}

/// Boxes side by side with `inline-block` straight in a cell (a newsletter's notes under its
/// products): one row of columns, side by side when they fit (three of 180 in 552, as in
/// Chrome), one under the other when they don't (in 300).
#[test]
fn inline_blocks_in_a_cell_are_columns_when_they_fit() {
    let html = r#"<table width="600"><tr><td style="padding:16px 24px"><div style="display:inline-block;vertical-align:top;width:180px"><h3>Free returns</h3><p>30 days.</p></div><div style="display:inline-block;vertical-align:top;width:180px"><h3>Two-year warranty</h3><p>On every lamp.</p></div><div style="display:inline-block;vertical-align:top;width:180px"><h3>Bulbs included</h3><p>Warm white.</p></div></td></tr></table>"#;
    let row = first_row(html);
    let Some(Block::Table(inner)) = row.first().and_then(|c| c.blocks.first()) else { panic!("columns in the cell: {row:?}") };
    let cols = inner.rows.first().cloned().unwrap_or_default();
    assert_eq!(cols.iter().map(|c| c.width).collect::<Vec<_>>(), [Some(Width::Px(180)); 3]);
    let (w, stack) = cell_widths(&cols, &column_widths(std::slice::from_ref(&cols)), &[], 552.0, 0.0);
    assert!(!stack && w.first().is_some_and(|w| (w - 180.0).abs() < 0.5), "{w:?}");
    let (_, stack) = cell_widths(&cols, &column_widths(std::slice::from_ref(&cols)), &[], 300.0, 0.0);
    assert!(stack, "too narrow: one under the other");
}

/// `f` given a `Ui` of a frame, its fonts ready.
fn with_ui<R>(f: impl FnOnce(&Ui) -> R) -> R {
    let ctx = egui::Context::default();
    let mut f = Some(f);
    let mut out = None;
    let mut done = ctx.run_ui(egui::RawInput::default(), |ui| {
        if let Some(f) = f.take() {
            out = Some(f(ui));
        }
    });
    // No renderer here: the font atlas's uploads let go deliberately.
    done.textures_delta.clear();
    out.expect("a frame")
}

/// A cell of pictures side by side, each `widths` pixels wide: content of a width known
/// without a font, so its columns can be held against Chrome's.
fn pictures(widths: &[u32]) -> Cell {
    let line = widths
        .iter()
        .map(|w| mime::Span {
            image: Some("cid:p@x".into()),
            image_size: Some(mime::ImageSize { width: Some(Width::Px(*w)), ..Default::default() }),
            ..Default::default()
        })
        .collect();
    Cell { blocks: vec![Block::Text(vec![line])], span: 1, ..Default::default() }
}

fn key() -> MessageKey {
    MessageKey { account: "a".into(), folder: "INBOX".into(), validity: 1, uid: 1 }
}

/// Columns that set no width, sized by what they hold as Chrome sizes them (CSS 2.1 §17.5.2.2):
/// pictures of 50, four of 100 and two of 150 side by side, a column each, `cellspacing=0`:
/// - in 600, more than their widest picture and less than all unwrapped: 50, 300, 250 (each its
///   widest, the rest in proportion to how much more it would take);
/// - one picture each in 600, all fit: 100, 200, 300 (the room left in proportion to each);
/// - a table that sets no width: as wide as they are, 300 (50, 100, 150);
/// - `cellspacing=4` in 600: 50, 289.33, 244.67.
///
/// They were 200 each, shared equally.
#[test]
fn columns_without_widths_are_sized_by_what_they_hold_as_in_chrome() {
    let wrapped = vec![pictures(&[50]), pictures(&[100; 4]), pictures(&[150; 2])];
    let one_each = vec![pictures(&[50]), pictures(&[100]), pictures(&[150])];
    let table = |row: &Vec<Cell>, spacing: u16, fit: bool| mime::layout::Table {
        rows: vec![row.clone()],
        width: (!fit).then_some(Width::Px(600)),
        spacing: Some(spacing),
        padding: Some(0),
        fit,
        ..Default::default()
    };
    let widths = |t: &mime::layout::Table, inner: f32| {
        let m = with_ui(|ui| measured(ui, &key(), t));
        cell_widths(&t.rows[0], &column_widths(&t.rows), &m.columns, inner, f32::from(t.spacing.unwrap_or(0)))
    };
    let (w, stack) = widths(&table(&wrapped, 0, false), 600.0);
    assert!(!stack && close(&w, &[50.0, 300.0, 250.0]), "{w:?}");
    let (w, _) = widths(&table(&one_each, 0, false), 600.0);
    assert!(close(&w, &[100.0, 200.0, 300.0]), "{w:?}");
    let t = table(&one_each, 0, true);
    let m = with_ui(|ui| measured(ui, &key(), &t));
    assert_eq!(m.whole.map(|f| f.max), Some(300.0));
    let (w, _) = widths(&t, 300.0);
    assert!(close(&w, &[50.0, 100.0, 150.0]), "{w:?}");
    // The row inside the table's own spacing (600 less 4 on each side).
    let (w, _) = widths(&table(&wrapped, 4, false), 592.0);
    assert!(close(&w, &[50.0, 289.33, 244.67]), "{w:?}");
    // Unmeasured, as before: shared equally.
    let (w, _) = cell_widths(&wrapped, &[], &[], 600.0, 0.0);
    assert!(close(&w, &[200.0, 200.0, 200.0]), "{w:?}");
}

/// Text measured in the fonts it's drawn in: a short column narrow, a long one wide, all of the
/// row used, none narrower than its widest word; side by side in 600, one under the other in a
/// narrow pane, as before. A table that sets no width is as wide as its words.
#[test]
fn text_columns_are_as_wide_as_their_words_ask() {
    let blocks = html_layout(
        r#"<table width="600" cellspacing="0" cellpadding="8"><tr><td>Qty</td><td>Our customers love this lamp: brass, hand-finished, with a linen shade, a dimmer and a two-year warranty on every part of it, shipped in two days.</td><td>Unit price</td></tr></table>"#,
    );
    let Some(Block::Table(t)) = blocks.first() else { panic!("a table: {blocks:?}") };
    let m = with_ui(|ui| measured(ui, &key(), t));
    let (w, stack) = cell_widths(&t.rows[0], &column_widths(&t.rows), &m.columns, 600.0, 0.0);
    assert!(!stack, "{w:?}");
    let [qty, text, price] = w.as_slice() else { panic!("{w:?}") };
    assert!(qty < price && price < text, "{w:?}");
    assert!((qty + text + price - 600.0).abs() < 0.5, "{w:?}");
    for (w, f) in w.iter().zip(&m.columns) {
        assert!(f.is_some_and(|f| *w + 0.5 >= f.min), "{w} {f:?}");
    }
    let (_, stack) = cell_widths(&t.rows[0], &column_widths(&t.rows), &m.columns, 300.0, 0.0);
    assert!(stack, "too narrow: one under the other");
    let blocks = html_layout(r#"<table cellspacing="0"><tr><td>Hello</td><td>there</td></tr></table><table width="600"><tr><td>a</td></tr></table>"#);
    let Some(Block::Table(t)) = blocks.first() else { panic!("a table: {blocks:?}") };
    assert!(t.fit);
    let m = with_ui(|ui| measured(ui, &key(), t));
    assert!(m.whole.is_some_and(|f| f.max > 20.0 && f.max < 150.0), "{m:?}");
}

/// A table of more words than are measured is left unmeasured, its room shared as before, and
/// quickly.
#[test]
fn a_huge_table_is_left_unmeasured() {
    let html = format!("<table width=\"600\"><tr><td>{}</td><td>b</td></tr></table>", "word ".repeat(30_000));
    let blocks = html_layout(&html);
    let Some(Block::Table(t)) = blocks.first() else { panic!("a table") };
    let started = std::time::Instant::now();
    let m = with_ui(|ui| measured(ui, &key(), t));
    assert!(started.elapsed().as_secs() < 10, "{:?}", started.elapsed());
    assert!(m.whole.is_none() && m.columns.first().is_some_and(Option::is_none), "{m:?}");
}

/// A table of values whose columns set their widths reads exactly as before: what's measured
/// changes nothing where the cells ask.
#[test]
fn a_table_of_values_with_widths_is_unchanged() {
    for html in [
        r#"<table width="600" cellspacing="0"><tr><td width="300">Item</td><td width="100">Qty</td><td width="200">Price</td></tr><tr><td>Brass lamp with a linen shade</td><td>2</td><td>$120.00</td></tr></table>"#,
        r#"<table width="600" cellspacing="2" cellpadding="4"><tr><td width="50%">Item</td><td width="20%">Qty</td><td width="30%">Price</td></tr><tr><td>Brass lamp</td><td>2</td><td>$120.00</td></tr></table>"#,
    ] {
        let blocks = html_layout(html);
        let Some(Block::Table(t)) = blocks.first() else { panic!("a table: {blocks:?}") };
        let m = with_ui(|ui| measured(ui, &key(), t));
        let columns = column_widths(&t.rows);
        for row in &t.rows {
            assert_eq!(cell_widths(row, &columns, &m.columns, 596.0, 2.0), cell_widths(row, &columns, &[], 596.0, 2.0), "{html}");
        }
    }
}

/// Round corners as asked, no more than half the box's shorter side (a pill), as CSS draws them.
#[test]
fn corners_are_no_rounder_than_half_the_shorter_side() {
    let rect = |w: f32, h: f32| egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(w, h));
    assert_eq!(corner(Some(4), rect(200.0, 44.0)), egui::CornerRadius::same(4));
    assert_eq!(corner(Some(200), rect(200.0, 44.0)), egui::CornerRadius::same(22));
    assert_eq!(corner(Some(30), rect(40.0, 100.0)), egui::CornerRadius::same(20));
    assert_eq!(corner(None, rect(40.0, 100.0)), egui::CornerRadius::ZERO);
    assert_eq!(corner(Some(9), rect(0.0, 0.0)), egui::CornerRadius::ZERO);
}

/// Word 12 lottery report: a title spanning the first two columns and a
/// one-column spacer must not shift ranking columns across subsequent rows.
#[test]
fn word_report_colspan_and_ranking_share_one_grid() {
    let html = r#"<table width="722" cellspacing="0"><tr>
      <td colspan="2" width="216">Title</td><td width="96"></td><td width="124"></td>
      <td width="110"></td><td width="32"></td><td width="72"></td><td width="72"></td>
    </tr><tr>
      <td width="81">Name</td><td width="134">Numbers</td><td width="96">1</td>
      <td width="124">2</td><td width="110">3</td><td width="32"></td>
      <td width="72">Correct</td><td width="72">Bonus</td>
    </tr></table>"#;
    let blocks = html_layout(html);
    let Some(Block::Table(table)) = blocks.first() else { panic!("{blocks:?}") };
    let columns = column_widths(&table.rows);
    let tracks = shared_column_widths(&table.rows, &columns, &[], 722.0, 0.0).expect("fully specified columns");
    assert_eq!(tracks.len(), 8);
    let first = widths_from_tracks(&table.rows[0], &tracks, 0.0).expect("title row");
    let second = widths_from_tracks(&table.rows[1], &tracks, 0.0).expect("data row");
    assert_eq!(first.len(), 7);
    assert_eq!(second.len(), 8);
    assert!((first[0] - second[0] - second[1]).abs() < 0.1);
    assert!((first[5] - second[6]).abs() < 0.1);
    assert!((first[6] - second[7]).abs() < 0.1);
    assert!((first.iter().sum::<f32>() - second.iter().sum::<f32>()).abs() < 0.1);
}

/// The extra width in a Word table must be shared across all eight fixed
/// columns, not dumped into the last ranking column.
#[test]
fn word_pixel_grid_scales_all_columns_proportionally() {
    let html = r#"<table width="722"><tr><td width="65">PP</td><td width="107">Numbers</td>
    <td width="76">1</td><td width="99">2</td><td width="88">3</td>
    <td width="26"></td><td width="58">Normal</td><td width="58">Bonus</td></tr></table>"#;
    let blocks = html_layout(html);
    let Some(Block::Table(table)) = blocks.first() else { panic!("{blocks:?}") };
    let cols = column_widths(&table.rows);
    let tracks = shared_column_widths(&table.rows, &cols, &[], 722.0, 0.0).expect("eight fixed columns");
    let original = [65.0_f32, 107.0, 76.0, 99.0, 88.0, 26.0, 58.0, 58.0];
    let total: f32 = original.iter().sum();
    assert_eq!(tracks.len(), original.len());
    for (actual, px) in tracks.iter().zip(original) {
        assert!((actual - px * 722.0 / total).abs() < 0.5, "{tracks:?}");
    }
    assert!((tracks.iter().sum::<f32>() - 722.0).abs() < 0.5);
}

/// Veeam status labels use 1% for a column that must not wrap. Its content
/// minimum is enforced without changing ordinary percentage-based layouts.
#[test]
fn veeam_one_percent_nowrap_keeps_status_label_on_one_line() {
    let html = r#"<table width="100%" cellspacing="0">
      <tr><td nowrap="" style="width:1%">Success</td>
      <td width="85">1</td><td width="85">Start time</td>
      <td width="85">8:00:16 AM</td><td>Details</td></tr></table>"#;
    let row = first_row(html);
    assert!(row[0].nowrap);
    let columns = column_widths(std::slice::from_ref(&row));
    // A premeasured 64px label simulates the font-specific measured maximum.
    let content = vec![Some(Fit { min: 64.0, max: 64.0 }), Some(Fit { min: 15.0, max: 15.0 }),
        Some(Fit { min: 75.0, max: 75.0 }), Some(Fit { min: 75.0, max: 75.0 }),
        Some(Fit { min: 45.0, max: 45.0 })];
    let (widths, stacked) = cell_widths(&row, &columns, &content, 900.0, 0.0);
    assert!(!stacked);
    assert!(widths[0] >= 64.0, "nowrap text squeezed: {widths:?}");
    let plain = first_row(r#"<table><tr><td style="width:1%">Success</td><td>Other</td></tr></table>"#);
    let plain_cols = column_widths(std::slice::from_ref(&plain));
    let (plain_width, _) = cell_widths(&plain, &plain_cols, &content[..2], 900.0, 0.0);
    assert!((plain_width[0] - 9.0).abs() < 0.5, "ordinary 1% unexpectedly expanded: {plain_width:?}");
}

/// Veeam Configuration Backup: 5 logical columns, four 125px data columns,
/// and a rightmost rowspan spacer with no explicit width. Row two omits that
/// fifth td because the spacer is still occupied by rowspan=3.
#[test]
fn veeam_configuration_trailing_rowspan_keeps_four_data_columns() {
    let html = r#"<table width="100%" style="border-collapse:collapse">
      <tr><td colspan="5">Tuesday, 17 November 2026</td></tr>
      <tr><td style="width:125px;white-space:nowrap">Start time</td>
          <td style="width:125px;white-space:nowrap">10:00:10 AM</td>
          <td style="width:125px;white-space:nowrap">Data size</td>
          <td style="width:125px;white-space:nowrap">43.1 MB</td>
          <td rowspan="3">&nbsp;</td></tr>
      <tr><td style="width:125px;white-space:nowrap">End time</td>
          <td style="width:125px">10:00:42 AM</td>
          <td style="width:125px">Backup size</td>
          <td style="width:125px">2.4 MB</td></tr>
      <tr><td style="width:125px">Duration</td><td style="width:125px">0:00:31</td>
          <td style="width:125px">Compression</td><td style="width:125px">18.09x</td></tr>
      <tr><td colspan="5">Details</td></tr>
      <tr><td style="width:125px;white-space:nowrap">Configuration catalog</td>
          <td style="width:125px">644</td><td style="width:125px">576.3 KB</td>
          <td style="width:125px">58.1 KB</td><td rowspan="5">&nbsp;</td></tr>
      <tr><td style="width:125px">Backups catalog</td><td>610</td>
          <td>2.1 MB</td><td>115.5 KB</td></tr>
      </table>"#;
    let blocks = html_layout(html);
    let Some(Block::Table(table)) = blocks.first() else { panic!("{blocks:?}") };
    let columns = column_widths(&table.rows);
    assert_eq!(columns.len(), 5);
    assert!(columns[4].is_none());
    let content = vec![
        Some(Fit { min: 160.0, max: 160.0 }),
        Some(Fit { min: 125.0, max: 125.0 }),
        Some(Fit { min: 125.0, max: 125.0 }),
        Some(Fit { min: 125.0, max: 125.0 }),
        Some(Fit { min: 1.0, max: 1.0 }),
    ];
    let tracks = shared_column_widths(&table.rows, &columns, &content, 1000.0, 0.0)
        .expect("trailing rowspan spacer should not disable shared grid");
    assert_eq!(tracks.len(), 5);
    assert!(tracks[0] >= 160.0, "nowrap label too narrow: {tracks:?}");
    for col in &tracks[1..4] {
        assert!((*col - 125.0).abs() < 0.5, "other data columns changed: {tracks:?}");
    }
    assert!(tracks[4] >= 400.0, "spacer must receive free width: {tracks:?}");
    let narrow_row = widths_from_tracks(&table.rows[2], &tracks, 0.0)
        .expect("rowspan occupies trailing fifth column");
    assert_eq!(narrow_row.len(), 4);
    assert!((narrow_row[3] - 125.0).abs() < 0.5);
    let header = widths_from_tracks(&table.rows[0], &tracks, 0.0)
        .expect("colspan should occupy all five tracks");
    assert_eq!(header.len(), 1);
    assert!((header[0] - 1000.0).abs() < 0.5);
}

/// Excel 9 PCM overtime report: cells spanning the yellow heading row
/// continue to occupy the same columns below it. The HTML omits their td
/// elements from the second source row.
#[test]
fn pcm_excel_rowspan_headers_keep_fifteen_column_grid() {
    let html = r#"<table width="1099" cellspacing="0" style="border-collapse:collapse">
      <tr><td rowspan="2">Area</td><td rowspan="2">WIP</td>
      <td rowspan="2">People</td><td rowspan="2">Applied</td>
      <td colspan="3">Time</td><td rowspan="2">Hours</td>
      <td rowspan="2" colspan="4">Remarks</td>
      <td colspan="2">Totals</td><td>X</td></tr>
      <tr><td>Start</td><td>End</td><td>Hours</td>
      <td colspan="2">Count</td><td>Y</td></tr>
      <tr><td>Black room</td><td>3</td><td>3</td><td>3</td>
      <td>15:30</td><td>22:30</td><td>6.5</td><td>19.5</td>
      <td colspan="4">Notes</td><td colspan="2">Totals</td><td>Z</td></tr>
    </table>"#;
    let blocks = html_layout(html);
    let Some(Block::Table(table)) = blocks.first() else { panic!("{blocks:?}") };
    let expanded = expand_rowspans(&table.rows);
    let span_count = |row: &[Cell]| row.iter().map(|c| usize::from(c.span.max(1))).sum::<usize>();
    assert_eq!(table.rows[1].iter().map(|c| usize::from(c.span)).sum::<usize>(), 6);
    assert!(expanded.iter().all(|r| span_count(r) == 15), "{expanded:?}");
    assert_eq!(expanded[1][0].blocks.len(), 0);
    assert_eq!(expanded[1][0].rowspan, 1);
    assert_eq!(expanded[1][4].blocks.len(), 1); // Start, at the 5th logical column
    let cols = column_widths(&expanded);
    let widths = shared_column_widths(&expanded, &cols, &[], 1099.0, 0.0);
    // A complete grid is preserved even when column widths are not fully
    // specified; normal content-based sizing remains available as fallback.
    assert!(widths.is_none() || widths.as_ref().is_some_and(|w| w.len() == 15));
}

/// PCM statistics and headcount table: a two-column header spans two rows
/// while the next source row has only thirteen columns.
#[test]
fn pcm_headcount_rowspan_two_column_header_stays_aligned() {
    let html = r#"<table><tr><td colspan="2" rowspan="2">Resigned</td>
      <td>2026-12</td><td>2026-11</td><td>2026-10</td></tr>
      <tr><td>85</td><td>84</td><td>85</td></tr>
      <tr><td colspan="2" rowspan="2">One</td><td>2025-12</td>
      <td>2025-11</td><td>2025-10</td></tr>
      <tr><td>75</td><td>74</td><td>73</td></tr></table>"#;
    let blocks = html_layout(html);
    let Some(Block::Table(table)) = blocks.first() else { panic!("{blocks:?}") };
    let rows = expand_rowspans(&table.rows);
    for r in &rows {
        assert_eq!(r.iter().map(|c| usize::from(c.span.max(1))).sum::<usize>(), 5, "{r:?}");
    }
    assert_eq!(rows[1][0].span, 2);
    assert!(rows[1][0].blocks.is_empty());
    assert_eq!(rows[3][0].span, 2);
}

/// Existing ordinary and non-spanning Word tables must remain untouched.
#[test]
fn no_rowspan_tables_keep_original_cell_layout() {
    let html = r#"<table width="400"><tr><td colspan="2">Title</td>
      <td>Three</td></tr><tr><td>One</td><td>Two</td><td>Three</td></tr></table>"#;
    let blocks = html_layout(html);
    let Some(Block::Table(t)) = blocks.first() else { panic!("{blocks:?}") };
    assert_eq!(expand_rowspans(&t.rows), t.rows);
}

/// Collapsed tables use one line per shared seam; the thicker sender edge wins.
/// Blank tables must not grow an invented grid of black borders.
#[test]
fn pcm_collapsed_border_seams_use_one_stroke() {
    use mime::layout::{Border, Borders};
    let black = Border { width: 1, color: [0, 0, 0] };
    let blue = Border { width: 2, color: [0, 0, 255] };
    let mut rows = vec![
        vec![
            Cell { span: 1, borders: Borders { right: Some(black), bottom: Some(black), ..Default::default() }, ..Default::default() },
            Cell { span: 1, borders: Borders { left: Some(blue), bottom: Some(black), ..Default::default() }, ..Default::default() },
        ],
        vec![
            Cell { span: 1, borders: Borders { top: Some(blue), right: Some(black), ..Default::default() }, ..Default::default() },
            Cell { span: 1, borders: Borders { top: Some(black), left: Some(black), ..Default::default() }, ..Default::default() },
        ],
    ];
    collapse_shared_borders(&mut rows);
    assert_eq!(rows[0][0].borders.right, Some(blue));
    assert_eq!(rows[0][1].borders.left, None);
    assert_eq!(rows[1][0].borders.right, Some(black));
    assert_eq!(rows[1][1].borders.left, None);
    assert_eq!(rows[0][0].borders.bottom, Some(blue));
    assert_eq!(rows[1][0].borders.top, None);
    assert_eq!(rows[0][1].borders.bottom, Some(black));
    assert_eq!(rows[1][1].borders.top, None);
    let mut blank = vec![vec![Cell { span: 1, ..Default::default() }, Cell { span: 1, ..Default::default() }]];
    collapse_shared_borders(&mut blank);
    assert!(blank[0].iter().all(|c| c.borders == Borders::default()));
}

/// Do not remove an edge across an unmatched colspan boundary: segments can
/// span only part of a neighbor and need separate border conflict handling.
#[test]
fn colspan_mismatch_does_not_remove_sender_borders() {
    use mime::layout::{Border, Borders};
    let line = Some(Border { width: 1, color: [0, 0, 0] });
    let mut rows = vec![
        vec![Cell { span: 2, borders: Borders { bottom: line, ..Default::default() }, ..Default::default() }],
        vec![
            Cell { span: 1, borders: Borders { top: line, ..Default::default() }, ..Default::default() },
            Cell { span: 1, borders: Borders { top: line, ..Default::default() }, ..Default::default() },
        ],
    ];
    collapse_shared_borders(&mut rows);
    assert_eq!(rows[0][0].borders.bottom, line);
    assert_eq!(rows[1][0].borders.top, line);
    assert_eq!(rows[1][1].borders.top, line);
}

/// Respect sender-selected border separation rather than collapsing all
/// tables merely because their HTML cellspacing was zero.
#[test]
fn collapse_mode_is_explicit_in_html_layout() {
    // A single-cell table may be simplified to ordinary content by the
    // parser. Two columns ensure this checks real table-layout metadata.
    let a = html_layout(r#"<table cellspacing="0"><tr><td>A</td><td>B</td></tr></table>"#);
    let b = html_layout(r#"<table style="border-collapse:collapse"><tr><td>A</td><td>B</td></tr></table>"#);
    let Some(Block::Table(separate)) = a.first() else { panic!("missing table") };
    let Some(Block::Table(collapsed)) = b.first() else { panic!("missing table") };
    assert!(!separate.collapse);
    assert!(collapsed.collapse);
}

/// Real QPLCHN Veeam report has a 1%-wide, nowrap "Success" column,
/// seven 85px columns, and an unmeasured trailing rowspan spacer. The grid
/// must give the label/name column its measured minimum, not just 1% of width.
#[test]
fn veeam_qplchn_percent_nowrap_shared_grid_uses_content_minimum() {
    let html = r#"<table width="100%" style="border-collapse:collapse">
      <tr><td colspan="9">Report</td></tr>
      <tr><td nowrap="" style="width:1%">Success</td>
      <td width="85">7</td><td width="85">Start time</td>
      <td width="85">1:00 AM</td><td width="85">Total size</td>
      <td width="85">4.6 TB</td><td width="85">Backup size</td>
      <td width="85">91.3 GB</td><td rowspan="3">&nbsp;</td></tr>
      <tr><td nowrap="">Warning</td><td>0</td><td>End time</td>
      <td>1:21 AM</td><td>Data read</td><td>136.9 GB</td>
      <td>Dedupe</td><td>1.0x</td></tr>
      <tr><td nowrap="">Error</td><td>0</td><td>Duration</td>
      <td>0:21</td><td>Transferred</td><td>88 GB</td>
      <td>Compression</td><td>1.4x</td></tr>
    </table>"#;
    let blocks = html_layout(html);
    let Some(Block::Table(t)) = blocks.first() else { panic!("{blocks:?}") };
    let expanded = expand_rowspans(&t.rows);
    let cols = column_widths(&expanded);
    assert_eq!(cols.len(), 9);
    assert_eq!(cols[0], Some(mime::layout::Width::Percent(1)));
    assert_eq!(cols[8], None);
    let mut measured = vec![Some(Fit { min: 85.0, max: 85.0 }); 9];
    measured[0] = Some(Fit { min: 245.0, max: 245.0 }); // QPLCHN04 (email server GW)
    measured[8] = Some(Fit { min: 1.0, max: 1.0 });
    let tracks = shared_column_widths(&expanded, &cols, &measured, 1500.0, 0.0).expect("trailing spacer");
    assert!(tracks[0] >= 245.0, "status and VM names may not wrap into vertical letters: {tracks:?}");
    assert!((tracks[1] - 85.0).abs() < 0.1, "{tracks:?}");
    assert!(tracks[8] > 500.0, "unused table width belongs to trailing spacer: {tracks:?}");
    let row = widths_from_tracks(&expanded[2], &tracks, 0.0).expect("rowspan continuation");
    assert_eq!(row.len(), 9);
}

/// Narrow percentage columns without a trailing spacer retain their normal
/// percentage behavior; the fix must not stretch arbitrary layout tables.
#[test]
fn ordinary_percent_table_does_not_force_content_minima() {
    let html = r#"<table width="100%"><tr><td width="1%">Status</td><td width="99%">Details</td></tr></table>"#;
    let row = first_row(html);
    let cols = column_widths(std::slice::from_ref(&row));
    let fit = vec![Some(Fit { min: 180.0, max: 180.0 }), Some(Fit { min: 100.0, max: 100.0 })];
    let tracks = shared_column_widths(&[row], &cols, &fit, 1000.0, 0.0).expect("fixed percentages");
    assert!(tracks[0] < 30.0, "ordinary percent column unexpectedly expanded: {tracks:?}");
}

/// The real MIS Web Buy email has 19 rows, 18 fully specified pixel columns,
/// width 1888px, and explicit cell borders. A later-painted cell must own
/// the common stroke so its background cannot hide the preceding cell's line.
#[test]
fn mis_purchase_18_column_collapsed_border_ownership() {
    use mime::layout::{Border, Borders, Width};
    let ink = Border { width: 1, color: [0, 0, 0] };
    let widths: [u32; 18] = [45, 79, 373, 218, 72, 72, 104, 79, 124, 94, 97, 72, 72, 99, 72, 72, 72, 72];
    assert_eq!(widths.iter().sum::<u32>(), 1888);
    let mut rows: Vec<Vec<Cell>> = (0..19).map(|_| widths.iter().map(|w| Cell {
        width: Some(Width::Px(*w)), span: 1, rowspan: 1,
        borders: Borders { top: Some(ink), right: Some(ink), bottom: Some(ink), left: Some(ink) },
        ..Default::default()
    }).collect()).collect();
    assert!(wide_bordered_grid(&rows, Some(Width::Px(1888))));
    collapse_wide_grid_borders(&mut rows);
    // Right/lower cells are drawn after left/upper backgrounds.
    assert_eq!(rows[0][0].borders.right, None);
    assert_eq!(rows[0][1].borders.left, Some(ink));
    assert_eq!(rows[0][0].borders.bottom, None);
    assert_eq!(rows[1][0].borders.top, Some(ink));
    // Outer borders are still present.
    assert_eq!(rows[0][0].borders.top, Some(ink));
    assert_eq!(rows[0][0].borders.left, Some(ink));
    assert_eq!(rows[18][17].borders.bottom, Some(ink));
    assert_eq!(rows[18][17].borders.right, Some(ink));
}

/// PCM, Veeam, lottery, narrow, merged and borderless tables retain the
/// existing renderer branch regardless of the email's sender/client.
#[test]
fn wide_grid_fallback_does_not_change_other_email_tables() {
    use mime::layout::{Border, Borders, Width};
    let ink = Border { width: 1, color: [0, 0, 0] };
    let make = |width: Option<Width>, styled: bool| -> Cell {
        Cell { width, span: 1, rowspan: 1,
            borders: if styled { Borders { right: Some(ink), bottom: Some(ink), ..Default::default() } } else { Borders::default() },
            ..Default::default()
        }
    };
    let narrow = vec![vec![make(Some(Width::Px(100)), true); 8]; 4];
    assert!(!wide_bordered_grid(&narrow, Some(Width::Px(800))));
    let borderless = vec![vec![make(Some(Width::Px(105)), false); 18]; 4];
    assert!(!wide_bordered_grid(&borderless, Some(Width::Px(1890))));
    let merged = vec![vec![make(Some(Width::Px(105)), true); 18]; 4];
    let mut merged = merged;
    merged[1][3].span = 2;
    assert!(!wide_bordered_grid(&merged, Some(Width::Px(1890))));
    let auto = vec![vec![make(None, true); 18]; 4];
    assert!(!wide_bordered_grid(&auto, Some(Width::Px(1890))));
}

/// MIS weekend duty EML: Excel 7-column 853px grid with widths sourced
/// from the first row and empty width-less cells in subsequent rows.
#[test]
fn excel_weekend_roster_keeps_continuous_grid_borders() {
    use mime::layout::{Border, Borders, Width};
    const TRACKS: [u32; 7] = [47, 88, 97, 87, 236, 155, 143];
    let black = Border { width: 1, color: [0, 0, 0] };
    let borders = Borders {
        top: Some(black), right: Some(black),
        bottom: Some(black), left: Some(black),
    };
    let mut rows: Vec<Vec<Cell>> = (0..5).map(|row| TRACKS.iter().map(|px| Cell {
        width: if row == 0 || row == 4 { Some(Width::Px(*px)) } else { None },
        span: 1, rowspan: 1, borders,
        ..Default::default()
    }).collect()).collect();
    assert!(excel_weekend_roster_grid(&rows, Some(Width::Px(853))));
    // Real EML uses style="width:639pt" and cell style widths in pt.
    // This is the actual path taken by MailCraft's CSS parser.
    let css_px = [46, 88, 97, 86, 236, 154, 142];
    let css_rows: Vec<Vec<Cell>> = (0..5).map(|row| css_px.iter().map(|px| Cell {
        width: if row == 0 || row == 4 { Some(Width::Px(*px)) } else { None },
        span: 1, rowspan: 1, borders,
        ..Default::default()
    }).collect()).collect();
    assert!(excel_weekend_roster_grid(&css_rows, Some(Width::Px(852))));
    collapse_wide_grid_borders(&mut rows);
    assert_eq!(rows[0][0].borders.right, None);
    assert_eq!(rows[0][1].borders.left, Some(black));
    assert_eq!(rows[0][0].borders.bottom, None);
    assert_eq!(rows[1][0].borders.top, Some(black));
    assert_eq!(rows[4][6].borders.bottom, Some(black));
    // Column sizes and all content remain unchanged by border ownership.
    assert_eq!(rows[0].iter().map(|c| c.width).collect::<Vec<_>>(),
        TRACKS.iter().map(|x| Some(Width::Px(*x))).collect::<Vec<_>>());
}

/// A seven-column table alone is not enough to activate the roster rule.
/// Protect normal Outlook, Veeam, PCM and Gmail tables from this code path.
#[test]
fn excel_weekend_roster_does_not_affect_other_seven_column_tables() {
    use mime::layout::{Border, Borders, Width};
    let b = Border { width: 1, color: [0, 0, 0] };
    let first = [47u32, 88, 97, 87, 236, 155, 143];
    let cell = |w| Cell { width: Some(Width::Px(w)), span: 1, rowspan: 1,
        borders: Borders { right: Some(b), bottom: Some(b), ..Default::default() },
        ..Default::default() };
    let rows = vec![first.iter().map(|w| cell(*w)).collect::<Vec<_>>(); 5];
    assert!(!excel_weekend_roster_grid(&rows, Some(Width::Px(854))));
    let mut other = rows.clone();
    other[0][4].width = Some(Width::Px(235));
    assert!(!excel_weekend_roster_grid(&other, Some(Width::Px(853))));
    let mut merged = rows.clone();
    merged[2][0].span = 2;
    assert!(!excel_weekend_roster_grid(&merged, Some(Width::Px(853))));
    let mut borderless = rows;
    borderless[0][0].borders = Borders::default();
    assert!(!excel_weekend_roster_grid(&borderless, Some(Width::Px(853))));
}

/// Exercise the actual HTML -> CSS -> MailCraft layout path rather than only
/// hand-built Cells: sender supplies 853 HTML pixels and 639pt CSS.
#[test]
fn excel_weekend_real_html_widths_select_safe_border_mode() {
    let mut html = String::from(r#"<table width="853" style="WIDTH:639pt;BORDER-COLLAPSE:collapse" cellspacing="0"><tbody>"#);
    for row in 0..5 {
        html.push_str("<tr>");
        for (i, pt) in [35, 66, 73, 65, 177, 116, 107].iter().enumerate() {
            if row == 2 && i == 2 {
                html.push_str(r#"<td style="BORDER-RIGHT:windowtext .5pt solid;BORDER-BOTTOM:windowtext .5pt solid">MIS</td>"#);
            } else {
                html.push_str(&format!(
                    r#"<td style="WIDTH:{pt}pt;BORDER-RIGHT:windowtext .5pt solid;BORDER-BOTTOM:windowtext .5pt solid">X</td>"#
                ));
            }
        }
        html.push_str("</tr>");
    }
    html.push_str("</tbody></table>");
    let blocks = html_layout(&html);
    let Some(Block::Table(t)) = blocks.first() else { panic!("{blocks:?}") };
    assert!(t.collapse);
    assert_eq!(t.width, Some(mime::layout::Width::Px(852)));
    assert!(excel_weekend_roster_grid(&t.rows, t.width), "{:?}", t.rows[0]);
}

/// The weekend duty table has five rendered rows but cells with different
/// text heights. Its seven vertical boundaries must span all five rows,
/// independent of the height of the text in each individual cell.
#[test]
fn excel_weekend_full_height_grid_overlay_geometry() {
    let widths = [46.0, 88.0, 97.0, 86.0, 236.0, 154.0, 142.0];
    let rows = [10.0, 34.0, 78.0, 109.0, 132.0, 156.0];
    let lines = excel_roster_grid_segments(15.0, &widths, &rows);
    assert_eq!(lines.len(), 14); // 6 horizontal, 8 full-height vertical
    let right = 15.0 + widths.iter().sum::<f32>();
    for (i, (a, b)) in lines.iter().take(6).enumerate() {
        assert_eq!(a.y, rows[i]);
        assert_eq!(b.y, rows[i]);
        assert_eq!(a.x, 15.0);
        assert_eq!(b.x, right);
    }
    for (a, b) in lines.iter().skip(6) {
        assert_eq!(a.x, b.x);
        assert_eq!(a.y, rows[0]);
        assert_eq!(b.y, rows[5]);
    }
    assert_eq!(lines[6].0.x, 15.0);
    assert_eq!(lines[13].0.x, right);
}

/// Never draw a synthetic overlay over unrelated or malformed tables.
#[test]
fn excel_roster_grid_overlay_rejects_unrelated_geometry() {
    assert!(excel_roster_grid_segments(0.0, &[100.0; 6], &[0.0; 6]).is_empty());
    assert!(excel_roster_grid_segments(0.0, &[100.0; 7], &[0.0; 5]).is_empty());
    assert!(excel_roster_grid_segments(0.0, &[100.0; 7], &[0.0, 10.0, 8.0, 20.0, 30.0, 40.0]).is_empty());
}
