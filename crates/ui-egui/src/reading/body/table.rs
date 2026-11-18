//! The reading pane: a laid-out message's table rows, their cells side by side or stacked, and
//! the widths of its columns.

use super::*;

mod measure;
pub(crate) use measure::{Fit, measured};

impl Body<'_> {
    /// A table row: its cells side by side, or one under the other when there's no room.
    /// `columns`: the widths the table's columns ask for (a width set in one row is the whole
    /// column's, as HTML sizes columns), and `content` what they hold ([`measure`]). Side by
    /// side, the row is as tall as its tallest cell (or the height one asks for, issue #47): each
    /// cell's background fills it, and its content sits at the top, in the middle or at the
    /// bottom, as its `valign` says. Heights are those the
    /// cells' content took the frame before (egui lays out once), so a row settles a frame on.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn row(
        &mut self,
        st: &mut UiState,
        ui: &mut Ui,
        row: &[mime::layout::Cell],
        columns: &[Option<mime::layout::Width>],
        content: &[Option<Fit>],
        tracks: Option<&[f32]>,
        width: f32,
        page: [u8; 3],
        room: Room,
        depth: usize,
    ) {
        let (widths, stack) = tracks.and_then(|t| widths_from_tracks(row, t, room.spacing))
            .map_or_else(|| cell_widths(row, columns, content, width, room.spacing), |w| (w, false));
        let heights_id = ui.next_auto_id().with("cell-heights");
        let known: Vec<f32> = ui.ctx().data(|d| d.get_temp(heights_id)).unwrap_or_default();
        // Each cell's padding: its own, else its table's.
        let pads: Vec<[f32; 4]> = row.iter().map(|c| c.padding.map_or([room.padding; 4], |p| p.map(f32::from))).collect();
        let outer: Vec<f32> = pads.iter().enumerate().map(|(i, p)| known.get(i).copied().unwrap_or(0.0) + p[0] + p[2]).collect();
        let asked: Vec<f32> = row.iter().map(|c| c.height.map_or(0.0, f32::from)).collect();
        let valigns: Vec<mime::layout::VAlign> = row.iter().map(|c| c.valign).collect();
        let (row_h, offsets) = if stack { (0.0, vec![0.0; row.len()]) } else { row_heights(&outer, &asked, &valigns) };
        let cell = |this: &mut Self, st: &mut UiState, ui: &mut Ui, i: usize, w: f32| -> f32 {
            let (Some(c), Some(pad)) = (row.get(i), pads.get(i).copied()) else { return 0.0 };
            let bg = theme::box_color(c.background, page, &st.tokens);
            let fill = bg.map_or(Color32::TRANSPARENT, |b| Color32::from_rgb(b[0], b[1], b[2]));
            let behind = c.background_image.as_deref().and_then(|src| this.background(st, ui.ctx(), src));
            // As tall as the row (or as it asks, stacked), its content where it says.
            let tall = if stack { asked.get(i).copied().unwrap_or(0.0) } else { row_h };
            let down = offsets.get(i).copied().unwrap_or(0.0);
            let mut content = 0.0;
            ui.allocate_ui_with_layout(vec2(w, 0.0), egui::Layout::top_down(Align::Min), |ui| {
                ui.set_width(w);
                // Its colour (and picture) drawn under its content once its size is known, with its
                // corners as round as it says.
                let framed = egui::Frame::new().inner_margin(room_margin(pad)).show(ui, |ui| {
                    let slot = ui.painter().add(egui::Shape::Noop);
                    let inner = (w - pad[1] - pad[3]).max(1.0);
                    ui.set_width(inner);
                    ui.set_min_height((tall - pad[0] - pad[2]).max(0.0));
                    ui.add_space(down);
                    let top = ui.cursor().top();
                    this.blocks(st, ui, &c.blocks, inner, bg.unwrap_or(page), c.align, depth + 1);
                    content = (ui.cursor().top() - top).max(0.0);
                    slot
                });
                let rect = framed.response.rect;
                ui.painter().set(framed.inner, backdrop(rect, c.radius, fill, behind.as_ref()));
                paint_borders(ui.painter(), rect, c.borders);
                // Boxes placed over it; the cell grows to hold any that reach past its end.
                let mut bottom = rect.bottom();
                for o in &c.overlays {
                    bottom = bottom.max(this.overlay(st, ui, o, rect, bg.unwrap_or(page), depth));
                }
                if bottom > rect.bottom() {
                    ui.add_space(bottom - rect.bottom());
                }
            });
            content
        };
        let mut took = vec![0.0; row.len()];
        if stack {
            for (i, c) in row.iter().enumerate() {
                if !c.blocks.is_empty()
                    && let Some(t) = took.get_mut(i)
                {
                    *t = cell(self, st, ui, i, width);
                }
            }
        } else {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = room.spacing;
                for (i, w) in widths.iter().enumerate() {
                    let h = cell(self, st, ui, i, *w);
                    if let Some(t) = took.get_mut(i) {
                        *t = h;
                    }
                }
            });
        }
        // Heights that changed (the first frame, a picture come, a narrower window): the row is
        // laid out again with them.
        if !ui.is_sizing_pass() && (took.len() != known.len() || took.iter().zip(&known).any(|(a, b)| (a - b).abs() > 0.5)) {
            ui.ctx().data_mut(|d| d.insert_temp(heights_id, took));
            ui.ctx().request_repaint();
        }
    }
}

/// Paint only the four explicitly styled cell edges.
fn paint_borders(painter: &egui::Painter, rect: egui::Rect, b: mime::layout::Borders) {
    let edge = |a, z, border: Option<mime::layout::Border>| {
        if let Some(border) = border {
            let c = border.color;
            painter.line_segment([a, z], egui::Stroke::new(f32::from(border.width), Color32::from_rgb(c[0], c[1], c[2])));
        }
    };
    edge(rect.left_top(), rect.right_top(), b.top);
    edge(rect.right_top(), rect.right_bottom(), b.right);
    edge(rect.left_bottom(), rect.right_bottom(), b.bottom);
    edge(rect.left_top(), rect.left_bottom(), b.left);
}

/// A box's colour and the picture behind it, filling `rect` with corners `radius` round, at most
/// half its shorter side (as CSS draws a radius too big for its box).
pub(super) fn backdrop(rect: egui::Rect, radius: Option<u16>, fill: Color32, behind: Option<&egui::TextureHandle>) -> egui::Shape {
    let round = corner(radius, rect);
    let mut shapes = Vec::new();
    if fill != Color32::TRANSPARENT {
        shapes.push(egui::Shape::Rect(egui::epaint::RectShape::filled(rect, round, fill)));
    }
    if let Some(tex) = behind {
        shapes.push(cover(tex, rect, round));
    }
    egui::Shape::Vec(shapes)
}

/// A radius in pixels as egui draws it on `rect`: no more than half its shorter side.
pub(crate) fn corner(radius: Option<u16>, rect: egui::Rect) -> egui::CornerRadius {
    let most = (rect.width().min(rect.height()) / 2.0).max(0.0);
    let r = radius.map_or(0.0, f32::from).min(most).min(f32::from(u8::MAX));
    egui::CornerRadius::same(r.round() as u8)
}

/// Expand HTML rowspan occupancy only for painting and measuring. Source
/// cells remain unchanged for EML storage, replies, and forwards.
/// Continuations cover the same logical columns without repeating text or
/// drawing an internal border through a merged cell.
pub(crate) fn expand_rowspans(rows: &[Vec<mime::layout::Cell>]) -> Vec<Vec<mime::layout::Cell>> {
    // Start column, rows remaining, original cell. Bounded by the HTML parser.
    let mut pending: Vec<(usize, u16, mime::layout::Cell)> = Vec::new();
    let mut expanded = Vec::with_capacity(rows.len());
    for row in rows {
        let mut output = Vec::with_capacity(row.len() + pending.len());
        let mut next = Vec::new();
        let mut column = 0usize;
        let mut occupied = pending.iter().peekable();
        let mut insert_pending = |at: &mut usize, out: &mut Vec<mime::layout::Cell>, next: &mut Vec<(usize, u16, mime::layout::Cell)>| {
            while occupied.peek().is_some_and(|(start, _, _)| *start == *at) {
                let (start, left, original) = occupied.next().expect("checked above");
                let mut continuation = original.clone();
                continuation.blocks.clear();
                continuation.overlays.clear();
                continuation.rowspan = 1;
                continuation.height = None;
                continuation.nowrap = false;
                continuation.borders.top = None;
                if *left > 1 {
                    continuation.borders.bottom = None;
                    next.push((*start, *left - 1, original.clone()));
                }
                out.push(continuation);
                *at += usize::from(original.span.max(1));
            }
        };
        for cell in row {
            insert_pending(&mut column, &mut output, &mut next);
            if column >= 64 { break; }
            let span = usize::from(cell.span.max(1));
            if cell.rowspan > 1 {
                next.push((column, cell.rowspan - 1, cell.clone()));
            }
            let mut top = cell.clone();
            if cell.rowspan > 1 {
                top.borders.bottom = None;
            }
            output.push(top);
            column = column.saturating_add(span);
        }
        insert_pending(&mut column, &mut output, &mut next);
        // If malformed HTML leaves gaps before a spanning cell, keep the
        // renderer safe rather than attempting to infer nonexistent cells.
        next.sort_by_key(|(start, _, _)| *start);
        pending = next;
        expanded.push(output);
    }
    expanded
}

/// Collapse only shared CSS borders, without introducing a border on
/// unstyled layout tables. The wider line wins; ties keep the preceding cell.
/// Unlike painting both strokes, this gives adjoining cells one common edge.
pub(crate) fn collapse_shared_borders(rows: &mut [Vec<mime::layout::Cell>]) {
    fn pick(a: Option<mime::layout::Border>, b: Option<mime::layout::Border>) -> Option<mime::layout::Border> {
        match (a, b) {
            (Some(a), Some(b)) => Some(if b.width > a.width { b } else { a }),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        }
    }
    // Vertical seams between neighboring cells in a row.
    for row in rows.iter_mut() {
        for i in 0..row.len().saturating_sub(1) {
            let (left, right) = row.split_at_mut(i + 1);
            let a = &mut left[i];
            let b = &mut right[0];
            a.borders.right = pick(a.borders.right, b.borders.left);
            b.borders.left = None;
        }
    }
    // Horizontal seams only where neighboring cells cover exactly the same
    // logical column range. Split/merged intersections retain their sender
    // edges, avoiding incorrectly removing portions of a colspan boundary.
    for ri in 0..rows.len().saturating_sub(1) {
        let (top_rows, bottom_rows) = rows.split_at_mut(ri + 1);
        let upper = &mut top_rows[ri];
        let lower = &mut bottom_rows[0];
        let mut left = 0usize;
        for above in upper.iter_mut() {
            let end = left + usize::from(above.span.max(1));
            let mut start = 0usize;
            for below in lower.iter_mut() {
                let below_end = start + usize::from(below.span.max(1));
                if start == left && below_end == end {
                    above.borders.bottom = pick(above.borders.bottom, below.borders.top);
                    below.borders.top = None;
                    break;
                }
                start = below_end;
                if start >= end { break; }
            }
            left = end;
        }
    }
}

/// Detect wide MSHTML purchase grids that explicitly specify every column
/// and paint a border on virtually every cell. These are susceptible to a
/// background painted later covering the preceding cell's collapsed stroke.
/// Keep this structural check narrow: ordinary, merged, auto-width, or
/// borderless tables continue to use the established #94 behavior.
pub(crate) fn wide_bordered_grid(rows: &[Vec<mime::layout::Cell>], table_width: Option<mime::layout::Width>) -> bool {
    if !matches!(table_width, Some(mime::layout::Width::Px(w)) if w >= 1200)
        || rows.len() < 3
        || rows.first().is_none_or(|r| r.len() < 16)
    {
        return false;
    }
    let count = rows[0].len();
    rows.iter().all(|r| {
        r.len() == count
            && r.iter().all(|c| c.span.max(1) == 1 && c.rowspan <= 1
                && matches!(c.width, Some(mime::layout::Width::Px(_))))
    }) && rows.first().is_some_and(|r| {
        r.iter().filter(|c| c.borders.right.is_some() && c.borders.bottom.is_some()).count() >= 16
    })
}

/// Excel-generated weekend duty roster: a fixed 853px, seven-column
/// border-collapsed grid. Unlike a wide purchase report, some blank cells
/// omit width attributes, so classify by the complete first-row pixel grid.
/// Keep the exception narrow to protect all existing table layouts.
pub(crate) fn excel_weekend_roster_grid(
    rows: &[Vec<mime::layout::Cell>],
    table_width: Option<mime::layout::Width>,
) -> bool {
    use mime::layout::Width;
    // The sender gives both width=853 and style="width:639pt" (=852px).
    // Each column likewise has both integer HTML width and point CSS width;
    // the existing parser correctly gives inline CSS precedence.
    const HTML_TRACKS: [u32; 7] = [47, 88, 97, 87, 236, 155, 143];
    const CSS_TRACKS: [u32; 7] = [46, 88, 97, 86, 236, 154, 142];
    matches!(table_width, Some(Width::Px(852 | 853)))
        && rows.len() >= 5
        && rows.iter().all(|r| {
            r.len() == HTML_TRACKS.len()
                && r.iter().all(|c| c.span.max(1) == 1 && c.rowspan <= 1)
        })
        && rows.first().is_some_and(|first| {
            (first.iter().zip(HTML_TRACKS).all(|(c, px)| c.width == Some(Width::Px(px)))
                || first.iter().zip(CSS_TRACKS).all(|(c, px)| c.width == Some(Width::Px(px))))
                && first.iter().all(|c| c.borders.right.is_some() && c.borders.bottom.is_some())
        })
}

/// The five-row Excel weekend roster needs vertical lines covering the full
/// rendered row height. Painting each individual cell edge before all row
/// heights have settled can leave short ticks instead of a continuous grid.
/// Paint the complete grid *after* all rows; do not change content layout.
pub(crate) fn excel_roster_grid_segments(
    x: f32,
    tracks: &[f32],
    row_edges: &[f32],
) -> Vec<(egui::Pos2, egui::Pos2)> {
    if tracks.len() != 7 || row_edges.len() != 6 {
        return Vec::new();
    }
    excel_complete_grid_segments(x, tracks, row_edges)
}

/// A full-height border overlay for well-formed, explicitly bordered fixed
/// Excel grids. It uses actual rendered row positions, never guessed heights.
pub(crate) fn excel_complete_grid_segments(
    x: f32,
    tracks: &[f32],
    row_edges: &[f32],
) -> Vec<(egui::Pos2, egui::Pos2)> {
    if tracks.len() < 2 || row_edges.len() < 2
        || !x.is_finite()
        || tracks.iter().any(|w| !w.is_finite() || *w <= 0.0)
        || row_edges.iter().any(|y| !y.is_finite())
        || row_edges.windows(2).any(|w| w[1] < w[0])
    {
        return Vec::new();
    }
    let top = row_edges[0];
    let bottom = *row_edges.last().expect("checked nonempty");
    let right = x + tracks.iter().sum::<f32>();
    let mut segments = Vec::with_capacity(row_edges.len() + tracks.len() + 1);
    for &y in row_edges {
        segments.push((egui::pos2(x, y), egui::pos2(right, y)));
    }
    let mut xx = x;
    segments.push((egui::pos2(xx, top), egui::pos2(xx, bottom)));
    for &w in tracks {
        xx += w;
        segments.push((egui::pos2(xx, top), egui::pos2(xx, bottom)));
    }
    segments
}

pub(crate) fn paint_excel_roster_grid(
    painter: &egui::Painter,
    x: f32,
    tracks: &[f32],
    row_edges: &[f32],
    border: mime::layout::Border,
) {
    let c = border.color;
    let stroke = egui::Stroke::new(f32::from(border.width), Color32::from_rgb(c[0], c[1], c[2]));
    for (a, b) in excel_complete_grid_segments(x, tracks, row_edges) {
        painter.line_segment([a, b], stroke);
    }
}

/// For wide bordered Excel/MSHTML grids only, assign a shared border to the
/// *later*-painted cell. This prevents the right/bottom neighbor's background
/// covering it. It does not change grid geometry, widths, or source HTML.
pub(crate) fn collapse_wide_grid_borders(rows: &mut [Vec<mime::layout::Cell>]) {
    fn pick(a: Option<mime::layout::Border>, b: Option<mime::layout::Border>) -> Option<mime::layout::Border> {
        match (a, b) {
            (Some(a), Some(b)) => Some(if b.width > a.width { b } else { a }),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        }
    }
    for row in rows.iter_mut() {
        for i in 0..row.len().saturating_sub(1) {
            let (left, right) = row.split_at_mut(i + 1);
            let border = pick(left[i].borders.right, right[0].borders.left);
            left[i].borders.right = None;
            right[0].borders.left = border;
        }
    }
    for i in 0..rows.len().saturating_sub(1) {
        let (above, below) = rows.split_at_mut(i + 1);
        for (upper, lower) in above[i].iter_mut().zip(below[0].iter_mut()) {
            let border = pick(upper.borders.bottom, lower.borders.top);
            upper.borders.bottom = None;
            lower.borders.top = border;
        }
    }
}

/// Resolve a fully specified Word/Excel column grid once for the whole table.
/// For partially defined grids use the existing content-driven layout.
pub(crate) fn shared_column_widths(
    rows: &[Vec<mime::layout::Cell>],
    columns: &[Option<mime::layout::Width>],
    content: &[Option<Fit>],
    width: f32,
    spacing: f32,
) -> Option<Vec<f32>> {
    let count = rows.iter().map(|row| row.iter().map(|c| usize::from(c.span.max(1))).sum::<usize>()).max()?;
    if count < 2 || count > 64 || columns.len() < count {
        return None;
    }
    // Veeam's fifth column contains a rowspan spacer with no width. The first
    // four columns specify pixel widths on every data row. Keep one shared
    // track for the trailing auto-width spacer instead of stretching column 4
    // whenever that spanning cell is absent from a subsequent HTML row.
    let trailing_auto = columns.get(..count)?.iter().take(count - 1).all(Option::is_some)
        && columns[count - 1].is_none();
    if !trailing_auto && columns.get(..count)?.iter().any(Option::is_none) {
        return None;
    }
    let cells: Vec<mime::layout::Cell> = columns.iter().take(count).enumerate().map(|(i, w)| {
        // Respect the measured width of a nowrap label such as
        // "Configuration catalog" rather than squeezing it to 125 px.
        let width = if trailing_auto {
            match (*w, content.get(i).and_then(|f| *f)) {
                (Some(mime::layout::Width::Px(px)), Some(fit)) =>
                    Some(mime::layout::Width::Px(px.max(fit.min.ceil().max(1.0) as u32))),
                // A Veeam 1%-wide status/name column can also be explicitly
                // nowrap. Use the measured minimum before the remaining width
                // is assigned to its trailing empty rowspan spacer.
                (Some(mime::layout::Width::Percent(pc)), Some(fit)) if fit.min > 0.0 =>
                    Some(mime::layout::Width::Px(
                        ((width * f32::from(pc) / 100.0).max(fit.min)).ceil().max(1.0) as u32,
                    )),
                _ => *w,
            }
        } else { *w };
        mime::layout::Cell { width, span: 1, ..Default::default() }
    }).collect();
    let (mut tracks, _) = cell_widths(&cells, columns, content, width, spacing);
    // Word may specify a 722px table but point-based cells totaling ~578px.
    // Expand those cells proportionally, rather than growing only the last one.
    // Other fixed-width tables keep the established layout behavior.
    if columns.iter().take(count).all(|w| matches!(w, Some(mime::layout::Width::Px(_)))) {
        let raw: Vec<f32> = columns.iter().take(count).filter_map(|w| {
            if let Some(mime::layout::Width::Px(px)) = w { Some(*px as f32) } else { None }
        }).collect();
        let raw_total = raw.iter().sum::<f32>();
        let available = (width - spacing * count.saturating_sub(1) as f32).max(1.0);
        if raw.len() == count && raw_total > 0.0 && available > raw_total * 1.15 {
            tracks = raw.iter().map(|px| px / raw_total * available).collect();
        }
    }
    (tracks.len() == count).then_some(tracks)
}

/// Give colspan cells the sum of the same column tracks every other row uses.
/// Internal gaps are inside a spanning cell, not between its siblings.
pub(crate) fn widths_from_tracks(row: &[mime::layout::Cell], tracks: &[f32], spacing: f32) -> Option<Vec<f32>> {
    let mut at = 0usize;
    let mut widths = Vec::with_capacity(row.len());
    for cell in row {
        let span = usize::from(cell.span.max(1));
        let end = at.checked_add(span)?;
        let slice = tracks.get(at..end)?;
        widths.push(slice.iter().sum::<f32>() + spacing * span.saturating_sub(1) as f32);
        at = end;
    }
    // A trailing rowspan cell appears only in the first source row; later
    // source rows legitimately end before the occupied trailing column.
    (at <= tracks.len() && at > 0).then_some(widths)
}

/// Each cell's width in a row `width` wide, `spacing` between the cells, `columns` the widths the
/// table's columns ask for and `content` what they hold ([`measure`]); and whether the cells
/// stack instead, one too narrow to read beside the others. What cells ask for (pixels, or a
/// share of the row as a browser takes it); the rest shared out by those that ask for nothing as
/// CSS 2.1's automatic table layout shares it (§17.5.2.2), when what they hold is measured: each
/// as wide as its content unwrapped if they all fit, the room left over in proportion to that,
/// else each as wide as its widest word or picture and what's left in proportion to how much
/// more each would take (a short column narrow, a long one wide); equally when it isn't. All of
/// it in proportion when they ask for more than there is.
pub(crate) fn cell_widths(
    row: &[mime::layout::Cell],
    columns: &[Option<mime::layout::Width>],
    content: &[Option<Fit>],
    width: f32,
    spacing: f32,
) -> (Vec<f32>, bool) {
    use mime::layout::Width;
    // The room between the cells side by side comes out of the row's width.
    let between = spacing * row.len().saturating_sub(1) as f32;
    let width = (width - between).max(1.0);
    let px = |w: Width| match w {
        Width::Px(px) => px as f32,
        Width::Percent(pc) => width * f32::from(pc) / 100.0,
    };
    // Widths: what cells (or their columns) ask for, the rest shared by the others; and what
    // each holds, from its columns' measures (those it spans, and the room between them).
    let mut col = 0usize;
    let (asked, fits): (Vec<Option<f32>>, Vec<Option<Fit>>) = row
        .iter()
        .map(|c| {
            let span = usize::from(c.span.max(1));
            let from_columns = || {
                let parts = columns.get(col..col + span)?;
                (parts.len() == span && parts.iter().all(Option::is_some)).then(|| parts.iter().flatten().map(|w| px(*w)).sum::<f32>())
            };
            let w = c.width.map(px).or_else(from_columns).map(|asked| {
                // A 1% width in automated reports is a preference, not
                // permission to split "Success" vertically when nowrap is set.
                if c.nowrap { asked.max(content.get(col).and_then(|f| *f).map_or(0.0, |f| f.max)) } else { asked }
            });
            let gaps = spacing * span.saturating_sub(1) as f32;
            let fit = content.get(col..col.saturating_add(span)).filter(|parts| parts.len() == span).and_then(|parts| {
                parts.iter().try_fold(Fit { min: gaps, max: gaps }, |a, p| p.map(|p| Fit { min: a.min + p.min, max: a.max + p.max }))
            });
            col = col.saturating_add(span);
            (w, fit)
        })
        .unzip();
    let fixed: f32 = asked.iter().flatten().sum();
    let free = asked.iter().filter(|a| a.is_none()).count();
    let room = width - fixed;
    // CSS's automatic layout, when every cell that asks for nothing is measured.
    let auto = free > 0 && asked.iter().zip(&fits).all(|(a, f)| a.is_some() || f.is_some());
    let unasked = || asked.iter().zip(&fits).filter(|(a, _)| a.is_none()).filter_map(|(_, f)| *f);
    let least: f32 = if auto { unasked().map(|f| f.min).sum() } else { MIN_COLUMN * free as f32 };
    let mut widths: Vec<f32> = if fixed > width || (free > 0 && room < least) {
        // Asked for more than there is: shared out in proportion (by what those that ask for
        // nothing can't do without, measured).
        let base = |a: &Option<f32>, f: &Option<Fit>| a.unwrap_or_else(|| if auto { f.map_or(MIN_COLUMN, |f| f.min.max(1.0)) } else { MIN_COLUMN });
        let total: f32 = asked.iter().zip(&fits).map(|(a, f)| base(a, f)).sum::<f32>().max(1.0);
        asked.iter().zip(&fits).map(|(a, f)| base(a, f) / total * width).collect()
    } else if auto {
        let (most, least_sum) = unasked().fold((0.0f32, 0.0f32), |(m, l), f| (m + f.max, l + f.min));
        let spread = most - least_sum;
        asked
            .iter()
            .zip(&fits)
            .map(|(a, f)| match (a, f) {
                (Some(a), _) => *a,
                // All of them unwrapped, and the room left over in proportion to that.
                (None, Some(f)) if most <= room => f.max + if most > 0.0 { (room - most) * f.max / most } else { (room - most) / free as f32 },
                // Each its widest word, and the rest in proportion to how much more it would take.
                (None, Some(f)) => {
                    f.min + if spread > 0.0 { (room - least_sum) * (f.max - f.min) / spread } else { (room - least_sum) / free as f32 }
                }
                (None, None) => 0.0,
            })
            .collect()
    } else {
        let each = if free > 0 { room / free as f32 } else { 0.0 };
        asked.iter().map(|a| a.unwrap_or(each)).collect()
    };
    // Room left over: shared by the cells that ask for a share of the row, in proportion, as a
    // browser shares it (three cells of 33% fill it equally); else the last cell's (a spacer next
    // to a wider cell asks for nothing more).
    let used: f32 = widths.iter().sum();
    let shares: f32 = row.iter().zip(&widths).filter(|(c, _)| matches!(c.width, Some(Width::Percent(_)))).map(|(_, w)| *w).sum();
    if used < width && shares > 0.0 {
        let grow = (width - used) / shares;
        for (w, c) in widths.iter_mut().zip(row) {
            if matches!(c.width, Some(Width::Percent(_))) {
                *w += *w * grow;
            }
        }
    } else if used < width && col >= columns.len()
        && let Some(last) = widths.last_mut()
    {
        *last += width - used;
    }
    // Stacked when a column is squeezed too narrow to read; one as narrow as it asks (a
    // business card's photo, issue #11), or as its content (measured), stays beside the others:
    // one measured is squeezed when it can't hold its widest word, or when the row has less room
    // than an even share of it would need and it would take more.
    let even = if free > 0 { room / free as f32 } else { f32::INFINITY };
    let squeezed = |w: f32, asked: Option<f32>, fit: Option<Fit>| {
        w < MIN_COLUMN
            && match (asked, fit) {
                (Some(a), _) => w + 0.5 < a,
                (None, Some(f)) if auto => w + 0.5 < f.min || (even < MIN_COLUMN && w + 0.5 < f.max),
                (None, _) => true,
            }
    };
    let stack =
        row.len() > 1 && widths.iter().zip(row).zip(asked.iter().zip(&fits)).any(|((w, c), (a, f))| squeezed(*w, *a, *f) && !c.blocks.is_empty());
    (widths, stack)
}

/// A row's height and how far down each cell's content starts: `outer` each cell's height with
/// its padding as last laid out, `asked` the height each asks for at least. The row is as tall as
/// the tallest of either; content sits at the top, in the middle or at the bottom of what's left.
pub(crate) fn row_heights(outer: &[f32], asked: &[f32], valign: &[mime::layout::VAlign]) -> (f32, Vec<f32>) {
    use mime::layout::VAlign;
    let tallest = outer.iter().chain(asked).fold(0.0f32, |a, b| a.max(*b));
    let offsets = outer
        .iter()
        .zip(valign)
        .map(|(h, v)| match v {
            VAlign::Top => 0.0,
            VAlign::Middle => ((tallest - h) / 2.0).max(0.0),
            VAlign::Bottom => (tallest - h).max(0.0),
        })
        .collect();
    (tallest, offsets)
}

/// A table's room: between and around its cells, and inside those that set none of their own.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Room {
    pub(super) spacing: f32,
    pub(super) padding: f32,
}

/// Room in pixels (top, right, bottom, left) as a frame's margin, held to what one takes.
pub(super) fn room_margin(p: [f32; 4]) -> Margin {
    let m = |v: f32| v.round().clamp(0.0, 120.0) as i8;
    Margin { top: m(p[0]), right: m(p[1]), bottom: m(p[2]), left: m(p[3]) }
}

/// The width each column of a table asks for: the first cell of one column that sets one, in any
/// row (HTML sizes a whole column so; only the first row set it in a report whose other rows then
/// lined up elsewhere).
pub(crate) fn column_widths(rows: &[Vec<mime::layout::Cell>]) -> Vec<Option<mime::layout::Width>> {
    const MAX_COLUMNS: usize = 64;
    let mut out: Vec<Option<mime::layout::Width>> = Vec::new();
    for row in rows {
        let mut col = 0usize;
        for c in row {
            let span = usize::from(c.span.max(1));
            if span == 1 && col < MAX_COLUMNS {
                if out.len() <= col {
                    out.resize(col + 1, None);
                }
                if let (Some(slot), Some(w)) = (out.get_mut(col), c.width)
                    && slot.is_none()
                {
                    *slot = Some(w);
                }
            }
            col = col.saturating_add(span);
        }
    }
    out
}

#[cfg(test)]
mod tests;
