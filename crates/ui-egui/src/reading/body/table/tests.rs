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
