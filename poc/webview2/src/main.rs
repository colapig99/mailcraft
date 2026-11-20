//! Standalone, Windows-only WebView2 rendering experiment.
//! Usage: mailcraft-webview2-poc.exe path-to-message.eml
//! F1: Chromium HTML view; F2: sanitized HTML source; Esc: exit.
//! NOT YET embedded in MailCraft. Never connects to Exchange or changes mail.
#[cfg(windows)]
mod windows {
    use std::{collections::HashSet, fs, path::{Path, PathBuf}, sync::{Arc, Mutex}};
    use mail_parser::MessageParser;
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use winit::{
        application::ApplicationHandler,
        event::{ElementState, WindowEvent},
        event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
        keyboard::{KeyCode, PhysicalKey},
        window::{Window, WindowId},
    };
    use wry::{WebView, WebViewBuilder, Rect};

    const MAX_EML_BYTES: u64 = 12 * 1024 * 1024;
    const MAX_FILES: usize = 500;
    const MAX_LAYOUT_REPORT_BYTES: usize = 450_000;
    const MAX_HTML_BYTES: usize = 1_500_000;
    const CSP: &str = "default-src 'none'; script-src 'none'; connect-src 'none'; img-src data:; style-src 'unsafe-inline'; font-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'";
    /// The only permitted data: navigations are the two HTML documents that
    /// this process generated and sanitized itself. Do not accept arbitrary
    /// data URLs, web URLs, or file URLs from untrusted mail content.
    fn allowed_navigation(url: &str, approved_base64: &[String; 2]) -> bool {
        if url == "about:blank" {
            return true;
        }
        let (header, body) = match url.split_once(',') {
            Some(parts) => parts,
            None => return false,
        };
        let header = header.to_ascii_lowercase();
        if !header.starts_with("data:text/html")
            || !header.ends_with(";base64")
            || body.len() > 2_800_000
        {
            return false;
        }
        approved_base64.iter().any(|expected| expected == body)
    }

    const HELP: &str = "<div style='position:sticky;top:0;background:#edf3ff;border-bottom:1px solid #aaa;padding:7px;font:13px Segoe UI,sans-serif;z-index:1000'>WebView2 comparison POC — F1: formatted HTML / F2: sanitized source / Esc: close. No network images or scripts.</div>";

    fn escape_html(input: &str) -> String {
        input.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
    }

    fn escape_attribute(input: &str) -> String {
        escape_html(input).replace('"', "&quot;").replace('\'', "&#39;")
    }

    /// Enumerate files only from the directory containing this executable.
    /// A web page never chooses a raw filesystem path.
    fn eml_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
        let mut files = Vec::new();
        let entries = fs::read_dir(dir).map_err(|e| format!("Cannot read EXE folder: {e}"))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("Cannot enumerate EML files: {e}"))?;
            let path = entry.path();
            let meta = fs::symlink_metadata(&path).map_err(|e| format!("Cannot inspect EML: {e}"))?;
            if !meta.file_type().is_file() { continue; }
            if path.extension().is_some_and(|s| s.to_string_lossy().eq_ignore_ascii_case("eml")) {
                files.push(path);
            }
        }
        files.sort_by_cached_key(|path| path.file_name().unwrap_or_default().to_string_lossy().to_lowercase());
        files.truncate(MAX_FILES);
        Ok(files)
    }

    fn option_list(files: &[PathBuf], selected: usize) -> String {
        files.iter().enumerate().map(|(i, path)| {
            let filename = path.file_name().unwrap_or_default().to_string_lossy();
            let sel = if i == selected { " selected" } else { "" };
            format!("<option value=\"{i}\"{sel}>{}</option>", escape_attribute(&filename))
        }).collect::<String>()
    }

    fn selected_index(command: &str, count: usize) -> Option<usize> {
        command.strip_prefix("select:")?.parse::<usize>().ok().filter(|index| *index < count)
    }

    fn shell_page(email_html: &str, files: &[PathBuf], selected: usize, mode: usize) -> Result<String, String> {
        // A tiny trusted UI script runs in the parent. Email HTML is placed in
        // a sandboxed srcdoc frame without allow-scripts or allow-same-origin.
        // No email markup can invoke the privileged Wry IPC bridge.
        let options = option_list(files, selected);
        let frame = escape_attribute(email_html);
        let modes = ["Filtered HTML", "Filtered HTML source", "Original Office HTML (diagnostic)", "Direct filtered document (experimental)", "Office compatibility (experimental)", "Layout diagnostics (filtered)", "Layout Diagnostic V2 (compare)"];
        let modes = modes.iter().enumerate().map(|(i, label)| {
            let chosen = if i == mode { " selected" } else { "" };
            format!("<option value=\"{i}\"{chosen}>{label}</option>")
        }).collect::<String>();
        // Only sanitized HTML may be inspected. Without allow-scripts, email
        // markup remains inert. allow-same-origin permits only the trusted
        // parent diagnostic script to read computed DOM geometry.
        let frame_sandbox = if mode == 5 || mode == 6 { "allow-same-origin" } else { "" };
        let diagnostics = if mode == 5 { layout_diagnostic_script() } else if mode == 6 { layout_comparison_script() } else { "" };
        let shell = format!(r#"<!doctype html><html><head><meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'nonce-mailcraft-local-ui'; style-src 'unsafe-inline'; frame-src 'self' about:; img-src data:; connect-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'">
<style>html,body{{margin:0;height:100%;overflow:hidden;font:13px Segoe UI,sans-serif;background:#fff}}
#layout{{display:flex;flex-direction:column;height:100vh}}header{{display:flex;align-items:center;gap:10px;flex-wrap:wrap;padding:9px;background:#edf3ff;border-bottom:1px solid #c4ccd7}}
select{{flex:1;min-width:190px;max-width:880px;padding:5px}}button{{padding:5px 12px}}iframe{{flex:1;min-height:0;width:100%;border:0;background:#fff}}</style></head>
<body><div id="layout"><header><strong>MailCraft WebView2 POC</strong>
<label for="emails">EML file:</label><select id="emails" aria-label="Select email">{options}</select>
<label for="display">Display:</label><select id="display" aria-label="HTML display mode">{modes}</select>
<button type="button" id="refresh">Refresh list</button>
<button type="button" id="export">Export filtered HTML</button>
<span>{} files — local EML only</span><span id="layout-status" role="status"></span><button type="button" id="layout-save" hidden>Save layout report</button></header>
<iframe id="mail-frame" title="Isolated email preview" sandbox="{frame_sandbox}" referrerpolicy="no-referrer" srcdoc="{frame}"></iframe></div>
<script nonce="mailcraft-local-ui">
document.getElementById('emails').addEventListener('change', e => window.ipc.postMessage('select:' + e.target.value));
document.getElementById('refresh').addEventListener('click', () => window.ipc.postMessage('refresh'));
document.getElementById('export').addEventListener('click', () => window.ipc.postMessage('export-filtered'));
document.getElementById('display').addEventListener('change', e => window.ipc.postMessage('mode:' + e.target.value));
{diagnostics}
</script></body></html>"#, files.len());
        if shell.len() > 2 * 1024 * 1024 { return Err("HTML plus local file picker exceeds the 2 MiB POC limit".into()); }
        Ok(shell)
    }


    /// Trusted parent-only JavaScript. The email iframe has no allow-scripts.
    /// We do not export email text, HTML, or links: only measured geometry and
    /// computed visual styles for diagnosing legacy Word/Excel table layout.
    fn layout_diagnostic_script() -> &'static str {
        r#"
(function () {
  const frame = document.getElementById('mail-frame');
  const status = document.getElementById('layout-status');
  const save = document.getElementById('layout-save');
  let report = null;
  function number(v) { return Number.isFinite(v) ? Math.round(v * 100) / 100 : null; }
  function rect(el) {
    const b = el.getBoundingClientRect();
    return {x:number(b.x), y:number(b.y), width:number(b.width), height:number(b.height)};
  }
  function textBounds(el) {
    const range = el.ownerDocument.createRange();
    range.selectNodeContents(el);
    const boxes = Array.from(range.getClientRects()).slice(0, 80);
    return {count:boxes.length, minX:number(Math.min(...boxes.map(b=>b.left))),
      maxX:number(Math.max(...boxes.map(b=>b.right)))};
  }
  function calculate() {
    try {
      const doc = frame.contentDocument;
      if (!doc || !doc.body) throw new Error('Sandbox did not permit sanitized DOM inspection');
      const tables = Array.from(doc.querySelectorAll('table')).slice(0, 30);
      let cellCount = 0, overflowCount = 0, stopped = false;
      const output = tables.map((table, tableIndex) => {
        const st = doc.defaultView.getComputedStyle(table);
        const rows = Array.from(table.rows).slice(0, 120).map((row, rowIndex) => {
          const cells = Array.from(row.cells).map((cell, columnIndex) => {
            if (cellCount >= 1200) { stopped = true; return null; }
            cellCount++;
            const cs = doc.defaultView.getComputedStyle(cell);
            const box = rect(cell);
            const text = textBounds(cell);
            const overflow = cell.scrollWidth > cell.clientWidth + 1 ||
              (text.maxX !== null && text.maxX > box.x + box.width + 2);
            if (overflow) overflowCount++;
            return {column:columnIndex, rowSpan:cell.rowSpan, colSpan:cell.colSpan, box,
              clientWidth:cell.clientWidth, scrollWidth:cell.scrollWidth, textLength:(cell.textContent || '').length,
              text, overflow, css:{width:cs.width, minWidth:cs.minWidth, maxWidth:cs.maxWidth,
              whiteSpace:cs.whiteSpace, fontFamily:cs.fontFamily, fontSize:cs.fontSize,
              lineHeight:cs.lineHeight, textAlign:cs.textAlign, borderLeft:cs.borderLeft,
              borderRight:cs.borderRight, borderTop:cs.borderTop, borderBottom:cs.borderBottom,
              backgroundColor:cs.backgroundColor, paddingLeft:cs.paddingLeft, paddingRight:cs.paddingRight}};
          }).filter(Boolean);
          return {row:rowIndex, box:rect(row), cells};
        });
        return {table:tableIndex, box:rect(table), css:{width:st.width, tableLayout:st.tableLayout,
          borderCollapse:st.borderCollapse, borderSpacing:st.borderSpacing}, rows};
      });
      report = {format:'mailcraft-layout-v1', mode:'sanitized',
        viewport:{width:frame.clientWidth, height:frame.clientHeight},
        tableCount:tables.length, cellCount, overflowCount, truncated:stopped, tables:output};
      status.textContent = tables.length + ' tables / ' + cellCount + ' cells / ' + overflowCount + ' potential overflows';
      save.hidden = false;
    } catch (e) {
      status.textContent = 'Layout diagnostic failed: ' + String(e);
      save.hidden = true;
    }
  }
  frame.addEventListener('load', () => requestAnimationFrame(() => requestAnimationFrame(calculate)));
  save.addEventListener('click', () => {
    if (!report) return;
    const value = JSON.stringify(report);
    if (value.length > 400000) {
      status.textContent = 'Report too large; refine cell limit before exporting';
      return;
    }
    window.ipc.postMessage('layout-report:' + value);
    status.textContent = 'Layout report requested; see PowerShell for filename';
  });
})();
"#
    }

    /// V2: compare two *sanitized* DOMs using identical iframe client widths.
    /// No email scripts or content extraction, no network, no live CSS changes.
    /// The second iframe is created by comparison_shell() with fixed bounds.
    fn layout_comparison_script() -> &'static str {
        r#"
(() => {
  const a = document.getElementById('mail-frame');
  const b = document.getElementById('comparison-frame');
  const status = document.getElementById('layout-status');
  const save = document.getElementById('layout-save');
  let latest = null;
  const round = v => Number.isFinite(v) ? Math.round(v * 100) / 100 : null;
  const rectangle = element => {
    const r = element.getBoundingClientRect();
    return {x:round(r.x),y:round(r.y),width:round(r.width),height:round(r.height)};
  };
  const record = frame => {
    const doc = frame.contentDocument;
    if (!doc || !doc.body) throw Error('Sanitized iframe inaccessible');
    let count=0, truncated=false;
    const tables=Array.from(doc.querySelectorAll('table')).slice(0,30).map((table,ti) => {
      const ts=doc.defaultView.getComputedStyle(table);
      const rows=Array.from(table.rows).slice(0,120).map((row,ri) => {
        const cells=Array.from(row.cells).map((cell,ci) => {
          if (count>=1200) {truncated=true; return null;} count++;
          const style=doc.defaultView.getComputedStyle(cell);
          const range=doc.createRange(); range.selectNodeContents(cell);
          const fragments=Array.from(range.getClientRects()).slice(0,60);
          const minX=fragments.length?Math.min(...fragments.map(x=>x.x)):null;
          const maxX=fragments.length?Math.max(...fragments.map(x=>x.right)):null;
          const box=rectangle(cell);
          const overlap = maxX!==null && maxX>box.x+box.width+2;
          return {column:ci,rowSpan:cell.rowSpan,colSpan:cell.colSpan,box, 
            measuredText:{x:round(minX),right:round(maxX),overflow:overlap},
            scrollWidth:cell.scrollWidth,clientWidth:cell.clientWidth,
            css:{fontFamily:style.fontFamily,fontSize:style.fontSize,
              whiteSpace:style.whiteSpace,textAlign:style.textAlign, width:style.width,
              paddingLeft:style.paddingLeft,paddingRight:style.paddingRight,
              borderTop:style.borderTop,borderRight:style.borderRight,
              borderBottom:style.borderBottom,borderLeft:style.borderLeft,
              backgroundColor:style.backgroundColor}};
        }).filter(Boolean);
        return {row:ri,cells};
      });
      return {table:ti,box:rectangle(table),tableLayout:ts.tableLayout,rows};
    });
    return {viewport:{width:frame.clientWidth,height:frame.clientHeight},
      tableCount:tables.length,cellCount:count,truncated,tables};
  };
  function comparison() {
    try {
      // Fix equal dimensions before measuring both documents.
      b.style.width = a.clientWidth + 'px';
      b.style.height = a.clientHeight + 'px';
      const filtered=record(a);
      const office=record(b);
      const changes=[];
      filtered.tables.forEach((table,ti) => {
        const other=office.tables[ti];
        if(!other) { changes.push({table:ti,kind:'table-missing'}); return; }
        if(Math.abs(table.box.width-other.box.width)>1)
          changes.push({table:ti,kind:'table-width',a:table.box.width,b:other.box.width});
        table.rows.forEach((row,ri) => {
          const otherRow=other.rows[ri]; if(!otherRow)return;
          row.cells.forEach((cell,ci) => {
            const c=otherRow.cells[ci]; if(!c)return;
            const changedStyles=Object.keys(cell.css).filter(k=>cell.css[k]!==c.css[k]);
            const dx=round(c.box.x-cell.box.x),dw=round(c.box.width-cell.box.width);
            if(changedStyles.length || Math.abs(dx)>1 || Math.abs(dw)>1 ||
                cell.measuredText.overflow!==c.measuredText.overflow) {
              if(changes.length<500)changes.push({table:ti,row:ri,column:ci,dx,dWidth:dw,
                styles:changedStyles,overflowA:cell.measuredText.overflow,
                overflowB:c.measuredText.overflow});
            }
          });
        });
      });
      latest={format:'mailcraft-layout-v2',stages:['sanitized-direct','sanitized-office-experiment'],
        filtered,office,changeCount:changes.length,changes};
      status.textContent='V2: '+filtered.cellCount+' cells, '+changes.length+' differences; click Save comparison';
      save.hidden=false;save.textContent='Save comparison';
    } catch (e) {
      status.textContent='V2 failed: '+String(e);
      save.hidden=true;
    }
  }
  let ready=0;
  function onload() {ready++; if(ready>=2) requestAnimationFrame(()=>requestAnimationFrame(comparison));}
  a.addEventListener('load',onload);
  b.addEventListener('load',onload);
  save.addEventListener('click',()=>{
    if(!latest)return;
    const json=JSON.stringify(latest);
    if(json.length>400000){status.textContent='V2 report too large';return;}
    window.ipc.postMessage('layout-report:'+json);
    status.textContent='V2 report requested; filename appears in PowerShell';
  });
})();
"#
    }

    fn comparison_shell(files: &[PathBuf], selected: usize, direct: &str, office: &str) -> Result<String, String> {
        let mut page=shell_page(direct,files,selected,6)?;
        let second = format!(
            "<iframe id=\"comparison-frame\" title=\"Sanitized Office experimental comparison\" sandbox=\"allow-same-origin\" referrerpolicy=\"no-referrer\" style=\"position:fixed;left:-10000px;top:0;visibility:hidden;width:100%;height:500px\" srcdoc=\"{}\"></iframe>",
            escape_attribute(office)
        );
        page=page.replacen("</iframe></div>",&format!("</iframe>{second}</div>"),1);
        if page.len()>2*1024*1024 { return Err("Comparison HTML exceeds WebView2 2 MiB limit".into()); }
        Ok(page)
    }

    /// Legacy Excel and Outlook write border declarations in the order
    /// "windowtext .5pt solid", which Chromium and CSS filters may reject.
    /// Rewrite only CSS declarations into standards-compliant syntax, then
    /// run the normal allowlist sanitizer. This is not a sanitizer bypass.
    fn normalize_office_declarations(input: &str) -> String {
        let colors = regex::Regex::new(r"(?i)\bwindowtext\b").expect("constant regex");
        let windows = regex::Regex::new(r"(?i)\bwindow\b").expect("constant regex");
        let recolored = colors.replace_all(input, "#000000");
        let recolored = windows.replace_all(&recolored, "#ffffff").into_owned();
        let border = regex::Regex::new(
            r"(?i)(border(?:-(?:left|right|top|bottom))?\s*:\s*)(#[0-9a-f]{6})\s+((?:\d+(?:\.\d+)?|\.\d+)(?:px|pt))\s+(solid|dotted|dashed|double)"
        ).expect("constant regex");
        let reordered = border.replace_all(&recolored, "${1}${3} ${4} ${2}").into_owned();
        // CSS property names are case-insensitive, but the sanitizer's
        // allowlist matches lowercase names. Legacy Office uses uppercase
        // declarations, which otherwise become style="" throughout tables.
        let prop = regex::Regex::new(r"(?i)(^|[;{])(\s*)([a-z][a-z0-9-]*)\s*:")
            .expect("constant regex");
        prop.replace_all(&reordered, |caps: &regex::Captures<'_>| {
            format!("{}{}{}:", &caps[1], &caps[2], caps[3].to_ascii_lowercase())
        }).into_owned()
    }

    /// Normalize only quoted style attribute values. Do not alter visible
    /// email text; Ammonia still validates all resulting CSS declarations.
    fn normalize_office_inline_styles(html: &str) -> String {
        let style = regex::Regex::new(r#"(?is)\bstyle\s*=\s*("[^"]*"|'[^']*')"#)
            .expect("constant regex");
        style.replace_all(html, |caps: &regex::Captures<'_>| {
            let whole = caps.get(0).expect("match").as_str();
            // Keep the attribute name and the original quoting intact.
            let quoted = caps.get(1).expect("quoted style value");
            let quote_start = quoted.start() - caps.get(0).expect("match").start();
            let raw = quoted.as_str();
            format!("{}{}{}{}", &whole[..quote_start], &raw[..1],
                normalize_office_declarations(&raw[1..raw.len()-1]), &raw[raw.len()-1..])
        }).into_owned()
    }

    /// Do not expose raw untrusted HTML to a browser. Preserve safe table
    /// geometry and a strict subset of inline CSS for the compatibility test.
    fn safe_fragment(html: &str) -> String {
        safe_fragment_with_office_fonts(html, false)
    }

    /// Opt-in Office experiment: keep safe Excel inline font runs.
    /// This is isolated from the existing sanitizer and default renderer.
    fn safe_fragment_with_office_fonts(html: &str, preserve_fonts: bool) -> String {
        let mut cleaner = ammonia::Builder::default();
        cleaner
            .rm_tags(&["img", "form", "input", "button", "svg", "video", "audio"])
            // Ammonia's defaults prioritize ordinary rich text, not complex
            // email grids. Explicitly retain safe structural table elements.
            .add_tags(&["table", "thead", "tbody", "tfoot", "tr", "td", "th",
                "colgroup", "col", "caption"])
            .add_generic_attributes(&[
                "style", "class", "width", "height", "align", "valign",
                "bgcolor", "cellpadding", "cellspacing", "border",
                "colspan", "rowspan", "span", "nowrap",
            ])
            .filter_style_properties(HashSet::from([
                "width", "min-width", "max-width", "height", "min-height",
                "border", "border-top", "border-right", "border-bottom", "border-left",
                "border-collapse", "border-spacing", "border-color", "border-width",
                "border-style", "padding", "padding-top", "padding-right",
                "padding-bottom", "padding-left", "margin", "margin-left",
                "margin-right", "margin-top", "margin-bottom", "background",
                "background-color", "color", "font-family", "font-size",
                "font-weight", "font-style", "line-height", "text-align",
                "vertical-align", "white-space", "word-break", "word-wrap",
                "table-layout", "display", "letter-spacing",
            ]))
            .url_relative(ammonia::UrlRelative::Deny);
        if preserve_fonts {
            cleaner.add_tags(&["font"])
                .add_generic_attributes(&["face", "size", "color"]);
        }
        cleaner.clean(&normalize_office_inline_styles(html)).to_string()
    }

    /// Excel 9 emits spacer-only mso-spacerun runs before the value of
    /// x:num cells. Chromium faithfully paints these nonbreaking spaces even
    /// when the Excel-only mso-spacerun property is unavailable, causing
    /// fixed-width numeric cells to overflow into their neighbours.
    /// Remove only leading, whitespace-only runs of explicitly numeric cells.
    /// The default HTML mode is unchanged.
    fn normalize_excel_numeric_spacers(html: &str) -> String {
        let numeric = regex::Regex::new(
            r#"(?is)(<td\b[^>]*\bx:num\s*=\s*["'][^"']+["'][^>]*>\s*)<span\b[^>]*mso-spacerun\s*:\s*yes[^>]*>(.*?)</span>"#
        ).expect("constant regex");
        let tags = regex::Regex::new(r"(?is)<[^>]*>").expect("constant regex");
        numeric.replace_all(html, |caps: &regex::Captures<'_>| {
            let text = tags.replace_all(&caps[2], "");
            let is_padding_only = text.chars().all(|c| c.is_whitespace())
                || text.replace("&nbsp;", "").replace("&#160;", "").trim().is_empty();
            if is_padding_only {
                caps[1].to_string()
            } else {
                caps[0].to_string()
            }
        }).into_owned()
    }

    /// Word/Outlook HTML sometimes provides a width attribute in pixels but
    /// a substantially smaller CSS width in points. Only the Office-labelled
    /// tables with a >10% disagreement are adjusted in the experimental mode.
    /// Never synthesize border edges: source border:none remains untouched.
    fn office_word_widths(fragment: &str) -> String {
        let matcher = regex::Regex::new(r#"(?is)<table\b[^>]*>"#).expect("constant regex");
        let attr = regex::Regex::new(r#"(?i)\bwidth="([0-9]{2,4})""#).expect("constant regex");
        let css = regex::Regex::new(r"(?i)\bwidth\s*:\s*([0-9]+(?:\.[0-9]+)?)pt").expect("constant regex");
        matcher.replace_all(fragment, |caps: &regex::Captures<'_>| {
            let tag = &caps[0];
            if !tag.contains("MsoNormalTable") { return tag.to_string(); }
            let Some(px) = attr.captures(tag).and_then(|m| m[1].parse::<u32>().ok()) else { return tag.to_string(); };
            let Some(pt) = css.captures(tag).and_then(|m| m[1].parse::<f32>().ok()) else { return tag.to_string(); };
            if !(80..=4000).contains(&px) || (px as f32) <= pt * (4.0/3.0) * 1.10 {
                return tag.to_string();
            }
            // Inline min-width only, no changes to cell text, rowspan,
            // colspan or border model. The attribute is already numeric.
            tag.replacen("style=\"", &format!("style=\"min-width:{px}px;"), 1)
        }).into_owned()
    }

    /// Office emails often keep their widths and borders in class selectors.
    /// Keep CSS-only text when it has no external fetch or markup. A full
    /// production sanitizer must replace this conservative POC gate.
    fn safe_stylesheets(html: &str) -> String {
        let Ok(re) = regex::Regex::new(r"(?is)<style\b[^>]*>(.*?)</style\s*>") else {
            return String::new();
        };
        let mut out = String::new();
        for cap in re.captures_iter(html) {
            let original = cap.get(1).map_or("", |m| m.as_str());
            // Outlook/Excel may wrap CSS in legacy HTML comment markers.
            // Strip those exact markers before applying the strict gate.
            let cleaned = original.replace("<!--", "").replace("-->", "");
            let cleaned = normalize_office_declarations(&cleaned);
            let css = cleaned.as_str();
            if css.len() > 128 * 1024 || out.len() + css.len() > 128 * 1024 {
                break;
            }
            let lower = css.to_ascii_lowercase();
            // Reject truncated rules (including a fake </style> inside a
            // quoted CSS value), not just obvious network-capable constructs.
            if css.matches('{').count() != css.matches('}').count()
                || css.contains('<')
                || lower.contains("url(")
                || lower.contains("@import")
                || lower.contains("expression(")
                || lower.contains("behavior:")
                || lower.contains("-moz-binding")
            {
                continue;
            }
            out.push_str(css);
            out.push('\n');
        }
        out
    }

    fn document(content: &str, stylesheet: &str) -> String {
        format!(
            "<!doctype html><html><head><meta charset='utf-8'><meta http-equiv='Content-Security-Policy' content=\"{CSP}\"><style>html{{background:white;color:#222}}body{{margin:8px;overflow:auto}}table{{max-width:none}}</style><style>{stylesheet}</style></head><body>{HELP}{content}</body></html>"
        )
    }

    pub(crate) fn pages(raw: &[u8]) -> Result<[String; 5], String> {
        let parsed = MessageParser::default().parse(raw).ok_or("Could not parse EML")?;
        let source = parsed.body_html(0)
            .map(|x| x.into_owned())
            .or_else(|| parsed.body_text(0).map(|x| format!("<pre>{}</pre>", escape_html(&x))))
            .ok_or("No HTML or text body in EML")?;
        if source.len() > MAX_HTML_BYTES {
            return Err("HTML body too large for safe WebView2 NavigateToString POC".into());
        }
        let safe = safe_fragment(&source);
        let css = safe_stylesheets(&source);
        // A real, clickable, no-JavaScript display switch works even when
        // the WebView child window has keyboard focus (winit F1/F2 may not).
        let compare = format!(
            "<div class='compare'><input type='checkbox' id='show-source'><label class='switch' for='show-source'> Show sanitized HTML source instead of rendered email</label><section class='rendered'>{safe}</section><pre class='source'>{}</pre></div>",
            escape_html(&safe)
        );
        let toggler_css = format!(
            "{css}\n.switch{{display:block;padding:7px;background:#f1f1f1;font:13px Segoe UI,sans-serif}}\n.source{{display:none;white-space:pre-wrap;overflow-wrap:anywhere}}\n#show-source:checked ~ .rendered{{display:none}}\n#show-source:checked ~ .source{{display:block}}"
        );
        let formatted = document(&compare, &toggler_css);
        // Compare Office HTML layout without the extra compare/section
        // wrappers. A filtered full document is handed straight to the
        // sandboxed srcdoc frame. This is opt-in; all existing emails retain
        // their default formatting path.
        let direct = document(&safe, &css);
        let office_source = normalize_excel_numeric_spacers(&source);
        let office_fragment = office_word_widths(&safe_fragment_with_office_fonts(&office_source, true));
        let office = document(&office_fragment, &css);
        let inspect = document(&format!(
            "<h3>Sanitized HTML generated by the POC</h3><pre style='white-space:pre-wrap'>{}</pre>",
            escape_html(&safe)
        ), "");
        // Original Office HTML is available only in an explicitly selected
        // local diagnostic mode. The trusted shell still enforces strict CSP
        // and uses a sandboxed iframe with no script/same-origin privileges.
        if formatted.len().max(inspect.len()).max(source.len()).max(direct.len()).max(office.len()) > 2 * 1024 * 1024 {
            return Err("Result exceeds WebView2 2 MiB HTML string limit".into());
        }
        Ok([formatted, inspect, source, direct, office])
    }

    /// One source of truth for the active mode, whether switching modes or
    /// changing the selected EML. Never silently reset to Filtered HTML.
    fn document_for_mode(
        views: &[String; 5], files: &[PathBuf], selected: usize, mode: usize
    ) -> Result<String, String> {
        if mode == 6 {
            comparison_shell(files, selected, &views[3], &views[4])
        } else if mode <= 5 {
            shell_page(&views[if mode == 5 { 3 } else { mode }], files, selected, mode)
        } else {
            Err("Invalid WebView2 display mode".into())
        }
    }

    enum ViewerEvent { Select(usize), Mode(usize), Refresh, ExportFiltered, LayoutReport(String) }

    struct Viewer {
        window: Option<Window>,
        webview: Option<WebView>,
        views: [String; 5],
        mode: usize,
        directory: PathBuf,
        files: Vec<PathBuf>,
        selected: usize,
        proxy: EventLoopProxy<ViewerEvent>,
        approved_base64: Arc<Mutex<[String; 2]>>,
    }

    impl Viewer {
        fn selected_document(&self) -> Result<String, String> {
            document_for_mode(&self.views, &self.files, self.selected, self.mode)
        }

        fn show_selected(&mut self) {
            let Some(path) = self.files.get(self.selected) else { return; };
            let result = (|| {
                let data = fs::read(path).map_err(|e| format!("Unable to read selected EML: {e}"))?;
                if data.len() as u64 > MAX_EML_BYTES { return Err("EML exceeds 12 MiB limit".into()); }
                let views = pages(&data)?;
                let html = document_for_mode(&views, &self.files, self.selected, self.mode)?;
                Ok::<_, String>((views, html))
            })();
            match result {
                Ok((views, html)) => {
                    self.views = views;
                    // Preserve selected display mode across EML changes.
                    eprintln!("EML switched: mode={} (0=filtered, 4=Office experiment, 5/6=diagnostics)", self.mode);
                    let encoded = [STANDARD.encode(html.as_bytes()), String::new()];
                    if let Ok(mut allowed) = self.approved_base64.lock() { *allowed = encoded; }
                    if let Some(webview) = &self.webview {
                        if let Err(e) = webview.load_html(&html) {
                            eprintln!("Could not display selected EML: {e}");
                        }
                    }
                }
                Err(e) => eprintln!("EML selection error: {e}"),
            }
        }
    }

    impl ApplicationHandler<ViewerEvent> for Viewer {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            if self.window.is_some() {
                return;
            }
            let attrs = Window::default_attributes()
                .with_title("MailCraft WebView2 POC — Office HTML comparison")
                .with_inner_size(winit::dpi::LogicalSize::new(1200.0, 800.0));
            let Ok(window) = event_loop.create_window(attrs) else {
                eprintln!("Unable to create the WebView2 host window");
                event_loop.exit();
                return;
            };
            let size = window.inner_size();
            eprintln!("WebView2 POC: host client size={}x{}, HTML bytes={}", size.width, size.height, self.views[self.mode].len());
            // Use the Wry/winit recommended Windows child view, sized to
            // the host's client coordinates rather than the screen position.
            let bounds = Rect {
                position: wry::dpi::PhysicalPosition::new(0, 0).into(),
                size: wry::dpi::PhysicalSize::new(size.width.max(1), size.height.max(1)).into(),
            };
            let initial = match self.selected_document() {
                Ok(html) => html,
                Err(error) => { eprintln!("Cannot prepare email selection UI: {error}"); event_loop.exit(); return; }
            };
            if let Ok(mut allowed) = self.approved_base64.lock() {
                *allowed = [STANDARD.encode(initial.as_bytes()), String::new()];
            }
            let approved_base64 = self.approved_base64.clone();
            let proxy = self.proxy.clone();
            let builder = WebViewBuilder::new()
                .with_bounds(bounds)
                .with_html(initial)
                // Only our trusted shell runs a tiny script. Mail content is
                // isolated inside an iframe sandbox with scripts forbidden.
                .with_ipc_handler(move |request| {
                    let body = request.body();
                    if body == "refresh" {
                        let _ = proxy.send_event(ViewerEvent::Refresh);
                    } else if body == "export-filtered" {
                        let _ = proxy.send_event(ViewerEvent::ExportFiltered);
                    } else if let Some(json) = body.strip_prefix("layout-report:") {
                        if json.len() <= MAX_LAYOUT_REPORT_BYTES {
                            let _ = proxy.send_event(ViewerEvent::LayoutReport(json.to_string()));
                        }
                    } else if let Some(index) = body.strip_prefix("mode:").and_then(|v| v.parse::<usize>().ok()) {
                        if index <= 6 { let _ = proxy.send_event(ViewerEvent::Mode(index)); }
                    } else if let Some(index) = body.strip_prefix("select:").and_then(|v| v.parse::<usize>().ok()) {
                        let _ = proxy.send_event(ViewerEvent::Select(index));
                    }
                })
                .with_devtools(false)
                .with_incognito(true)
                .with_general_autofill_enabled(false)
                .with_on_page_load_handler(|_event, url| eprintln!("WebView2 page load callback URL: {url}"))
                .with_navigation_handler(move |url| {
                    // Wry's with_html/load_html internally navigate to an
                    // encoded data:text/html URL on this Windows backend.
                    // Accept ONLY the exact two sanitized POC documents.
                    let ok = approved_base64.lock().is_ok_and(|list| allowed_navigation(&url, &*list)) || url == "about:srcdoc";
                    let scheme = url.split(':').next().unwrap_or("unknown");
                    eprintln!("WebView2 navigation: scheme={scheme}, length={}, allowed={ok}", url.len());
                    ok
                });
            let webview = match builder.build_as_child(&window) {
                Ok(webview) => webview,
                Err(e) => {
                    eprintln!("WebView2 child creation failed: {e}. Check Edge WebView2 Runtime.");
                    event_loop.exit();
                    return;
                }
            };
            if let Err(e) = webview.set_bounds(Rect {
                position: wry::dpi::PhysicalPosition::new(0, 0).into(),
                size: wry::dpi::PhysicalSize::new(size.width.max(1), size.height.max(1)).into(),
            }) {
                eprintln!("WebView2 initial child bounds failed: {e}");
            }
            self.webview = Some(webview);
            self.window = Some(window);
        }

        fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: ViewerEvent) {
            match event {
                ViewerEvent::Mode(mode) if mode <= 6 => {
                    self.mode = mode;
                    if let Ok(html) = self.selected_document() {
                        if let Ok(mut approved) = self.approved_base64.lock() {
                            *approved = [STANDARD.encode(html.as_bytes()), String::new()];
                        }
                        if let Some(webview) = self.webview.as_ref() {
                            if let Err(err) = webview.load_html(&html) {
                                eprintln!("HTML display mode switch failed: {err}");
                            }
                        }
                    }
                }
                ViewerEvent::Select(index) if index < self.files.len() => {
                    if self.selected != index { self.selected = index; self.show_selected(); }
                }
                ViewerEvent::LayoutReport(json) => {
                    let verified = serde_json::from_str::<serde_json::Value>(&json)
                        .ok().filter(|value| {
                            ((value.get("format").and_then(|s| s.as_str()) == Some("mailcraft-layout-v1")
                                && value.get("mode").and_then(|s| s.as_str()) == Some("sanitized")
                                && value.get("tables").is_some_and(|v| v.is_array()))
                                || (value.get("format").and_then(|s| s.as_str()) == Some("mailcraft-layout-v2")
                                && value.get("stages").is_some_and(|v| v.is_array())
                                && value.get("filtered").is_some_and(|v| v.is_object())
                                && value.get("office").is_some_and(|v| v.is_object())))

                                && json.len() <= MAX_LAYOUT_REPORT_BYTES
                        });
                    if !(self.mode == 5 || self.mode == 6) || verified.is_none() {
                        eprintln!("Rejected unexpected layout diagnostic report");
                    } else if let Some(file) = self.files.get(self.selected) {
                        let name = file.file_name().unwrap_or_default().to_string_lossy();
                        let mut exported = false;
                        for number in 1..=99 {
                            let output = self.directory.join(format!("{name}.layout-{number:02}.json"));
                            match fs::OpenOptions::new().write(true).create_new(true).open(&output) {
                                Ok(mut out) => {
                                    use std::io::Write;
                                    match out.write_all(json.as_bytes()) {
                                        Ok(()) => { eprintln!("Layout diagnostic saved: {}", output.display()); exported = true; }
                                        Err(e) => eprintln!("Layout diagnostic write failed: {e}"),
                                    }
                                    break;
                                }
                                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                                Err(e) => { eprintln!("Layout diagnostic export failed: {e}"); break; }
                            }
                        }
                        if !exported { eprintln!("No layout report created"); }
                    }
                }
                ViewerEvent::ExportFiltered => {
                    // Explicit local diagnostic export. It writes only the
                    // already sanitized, script-disabled HTML document and
                    // never the raw untrusted EML or remote content.
                    if let Some(file) = self.files.get(self.selected) {
                        let name = file.file_name().unwrap_or_default().to_string_lossy();
                        let output = self.directory.join(format!("{name}.filtered.html"));
                        match fs::OpenOptions::new().write(true).create_new(true).open(&output) {
                            Ok(mut out) => {
                                use std::io::Write;
                                match out.write_all(self.views[3].as_bytes()) {
                                    Ok(()) => eprintln!("Sanitized HTML exported to: {}", output.display()),
                                    Err(e) => eprintln!("Failed writing diagnostic HTML: {e}"),
                                }
                            }
                            Err(e) => eprintln!("Export not created (existing file or permission): {} — {e}", output.display()),
                        }
                    }
                }
                ViewerEvent::Refresh => {
                    match eml_files(&self.directory) {
                        Ok(files) => {
                            let old_path = self.files.get(self.selected).cloned();
                            self.files = files;
                            self.selected = old_path.and_then(|p| self.files.iter().position(|q| q == &p)).unwrap_or(0);
                            if !self.files.is_empty() { self.show_selected(); }
                        }
                        Err(e) => eprintln!("EML refresh failed: {e}"),
                    }
                }
                _ => {}
            }
        }

        fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
            match event {
                WindowEvent::CloseRequested => event_loop.exit(),
                WindowEvent::Resized(size) => {
                    if let Some(webview) = self.webview.as_ref() {
                        let rect = Rect {
                            position: wry::dpi::PhysicalPosition::new(0, 0).into(),
                            size: wry::dpi::PhysicalSize::new(size.width.max(1), size.height.max(1)).into(),
                        };
                        if let Err(e) = webview.set_bounds(rect) {
                            eprintln!("WebView2 resized bounds failed: {e}");
                        }
                    }
                },
                WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                    match event.physical_key {
                        PhysicalKey::Code(KeyCode::Escape) => event_loop.exit(),
                        PhysicalKey::Code(KeyCode::F1) | PhysicalKey::Code(KeyCode::F2) | PhysicalKey::Code(KeyCode::F3) => {
                            let next = match event.physical_key { PhysicalKey::Code(KeyCode::F1) => 0, PhysicalKey::Code(KeyCode::F2) => 1, _ => 2 };
                            if next != self.mode {
                                self.mode = next;
                                if let Some(webview) = self.webview.as_ref() {
                                    if let Ok(html) = self.selected_document() {
                                        if let Ok(mut approved) = self.approved_base64.lock() {
                                            *approved = [STANDARD.encode(html.as_bytes()), String::new()];
                                        }
                                        if let Err(e) = webview.load_html(&html) {
                                            eprintln!("Unable to change display mode: {e}");
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    pub(super) fn run() -> Result<(), String> {
        let executable = std::env::current_exe().map_err(|e| format!("Cannot locate EXE: {e}"))?;
        let directory = executable.parent().ok_or("Cannot locate EXE directory")?.to_path_buf();
        let files = eml_files(&directory)?;
        if files.is_empty() {
            return Err(format!("No .eml files in EXE folder: {}", directory.display()));
        }
        let requested = std::env::args_os().nth(1).map(PathBuf::from);
        let selected = requested.as_ref().and_then(|p| {
            let filename = p.file_name()?;
            files.iter().position(|candidate| candidate.file_name() == Some(filename))
        }).unwrap_or(0);
        let raw = fs::read(&files[selected]).map_err(|e| format!("Cannot read EML: {e}"))?;
        if raw.len() as u64 > MAX_EML_BYTES { return Err("EML exceeds 12 MiB limit".into()); }
        let views = pages(&raw)?;
        let event_loop = EventLoop::<ViewerEvent>::with_user_event().build()
            .map_err(|e| format!("Window event loop failed: {e}"))?;
        let proxy = event_loop.create_proxy();
        eprintln!("Loaded {} EML files from {}", files.len(), directory.display());
        event_loop.run_app(&mut Viewer {
            window: None, webview: None, views, mode: 0, directory, files,
            selected, proxy, approved_base64: Arc::new(Mutex::new([String::new(), String::new()])),
        }).map_err(|e| format!("WebView2 event loop failed: {e}"))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// Word 12 lottery email uses the shorthand "border:solid
        /// windowtext 1.0pt", unlike Excel's "windowtext .5pt solid".
        /// Keep sender-defined unbordered spacer cells genuinely unbordered.
        #[test]
        fn switching_eml_maintains_explicit_display_mode() {
            let files = vec![PathBuf::from("first.eml"), PathBuf::from("second.eml")];
            let views_a = [
                "<p>default A</p>".into(), "<pre>source A</pre>".into(),
                "<p>raw A</p>".into(), "<p>direct A</p>".into(),
                "<p>office A</p>".into()
            ];
            let views_b = [
                "<p>default B</p>".into(), "<pre>source B</pre>".into(),
                "<p>raw B</p>".into(), "<p>direct B</p>".into(),
                "<p>office B</p>".into()
            ];
            let before = document_for_mode(&views_a, &files, 0, 4).unwrap();
            let after = document_for_mode(&views_b, &files, 1, 4).unwrap();
            assert!(before.contains("office A"));
            assert!(after.contains("office B"));
            assert!(!after.contains("default B"));
            assert!(after.contains(r#"<option value="4" selected>Office compatibility (experimental)</option>"#));
            let compare = document_for_mode(&views_b, &files, 1, 6).unwrap();
            assert!(compare.contains("mailcraft-layout-v2"));
            assert!(compare.contains(r#"<option value="6" selected>Layout Diagnostic V2 (compare)</option>"#));
            assert!(document_for_mode(&views_b, &files, 1, 7).is_err());
        }

        #[test]
        fn v2_comparison_uses_only_sanitized_inputs_and_no_mail_scripts() {
            let files=vec![PathBuf::from("comparison.eml")];
            let doc = comparison_shell(&files,0,"<p>Filtered</p>","<p>Office</p>").unwrap();
            assert!(doc.contains("mail-frame"));
            assert!(doc.contains("comparison-frame"));
            assert!(doc.contains("mailcraft-layout-v2"));
            assert!(doc.contains("getComputedStyle"));
            assert!(doc.contains("getBoundingClientRect"));
            assert!(doc.contains(r#"sandbox="allow-same-origin""#));
            assert!(!doc.contains("allow-scripts"));
            assert!(doc.contains("default-src 'none'"));
        }

        #[test]
        fn layout_diagnostics_only_on_explicit_sanitized_mode() {
            let files = vec![PathBuf::from("sample.eml")];
            let default = shell_page("<table><tr><td>A</td></tr></table>", &files, 0, 0).unwrap();
            let diag = shell_page("<table><tr><td>A</td></tr></table>", &files, 0, 5).unwrap();
            assert!(default.contains(r#"sandbox="""#), "{default}");
            assert!(!default.contains("layout-report:"));
            assert!(diag.contains(r#"sandbox="allow-same-origin""#), "{diag}");
            assert!(!diag.contains("allow-scripts"));
            assert!(diag.contains("layout-report:"));
            assert!(diag.contains("getBoundingClientRect"));
            assert!(diag.contains("getComputedStyle"));
            assert!(diag.contains("scrollWidth"));
            assert!(diag.contains("textLength"));
            assert!(!diag.contains("innerHTML"));
        }

        #[test]
        fn numeric_excel_padding_cannot_bleed_into_adjacent_fixed_cells() {
            let source = r#"<table><tr>
              <td class="xl103" x:num="3"><span style="mso-spacerun: yes"><font face="????">&nbsp;</font><font face="????">&nbsp;</font> </span>3 </td>
              <td class="xl109">15:30</td>
              <td class="xl104" x:num="6"><span style="mso-spacerun: yes">&nbsp;&nbsp;&nbsp; </span>6 </td>
              <td>22:30</td></tr></table>"#;
            let cleaned = normalize_excel_numeric_spacers(source);
            assert!(!cleaned.contains("mso-spacerun"), "{cleaned}");
            assert!(cleaned.contains(r#"x:num="3">3 </td>"#), "{cleaned}");
            assert!(cleaned.contains(r#"x:num="6">6 </td>"#), "{cleaned}");
            assert!(cleaned.contains("15:30") && cleaned.contains("22:30"));
            // Preserve meaningful spaces and unmarked nonnumeric cells.
            let text = r#"<td><span style="mso-spacerun: yes">&nbsp;&nbsp;</span>Text</td>"#;
            assert_eq!(normalize_excel_numeric_spacers(text), text);
            let significant = r#"<td x:num="7"><span style="mso-spacerun: yes">ABC</span>7</td>"#;
            assert_eq!(normalize_excel_numeric_spacers(significant), significant);
        }

        #[test]
        fn office_font_preservation_is_opt_in() {
            let html = r#"<table><tr><td><font class="font40" face="MingLiU" size="2" color="blue">22:30</font></td></tr></table><script>alert(1)</script>"#;
            let normal = safe_fragment(html);
            let office = safe_fragment_with_office_fonts(html, true);
            assert!(!normal.contains("<font"), "{normal}");
            assert!(office.contains("<font"), "{office}");
            assert!(office.contains(r#"class="font40""#), "{office}");
            assert!(!office.contains("alert(1)"), "{office}");
        }

        #[test]
        fn word_only_width_reconciliation_never_changes_other_tables() {
            let lottery = r#"<table class="MsoNormalTable" width="588" style="width:352.7pt;border-collapse:collapse"><tr><td style="border:none">X</td></tr></table>"#;
            let converted = office_word_widths(lottery);
            assert!(converted.contains("min-width:588px"), "{converted}");
            assert!(converted.contains("border:none"));
            let pcm = r#"<table width="1099" style="width:824pt;table-layout:fixed"><tr><td>X</td></tr></table>"#;
            assert_eq!(office_word_widths(pcm), pcm);
            let unrelated = lottery.replace("MsoNormalTable", "ordinary");
            assert_eq!(office_word_widths(&unrelated), unrelated);
        }

        #[test]
        fn direct_filtered_document_removes_compare_wrappers() {
            let email = b"From: test@example.invalid\r\nTo: test@example.invalid\r\nMIME-Version: 1.0\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<table style='table-layout:fixed'><tr><td>Test</td></tr></table>";
            let pages = pages(email).expect("valid email");
            assert!(pages[0].contains("class='compare'"));
            assert!(pages[3].contains("<table"));
            assert!(!pages[3].contains("class='compare'"));
            assert!(!pages[3].contains("class='rendered'"));
            assert!(pages[3].contains("default-src 'none'"));
            let names = vec![PathBuf::from("sample.eml")];
            let shell = shell_page(&pages[3], &names, 0, 3).unwrap();
            assert!(shell.contains(r#"<option value="3" selected>Direct filtered document (experimental)</option>"#));
            assert!(shell.contains(r#"sandbox="""#));
        }

        #[test]
        fn export_filtered_html_is_explicit_and_never_auto_sends_content() {
            let file = vec![PathBuf::from("PCM.eml")];
            let shell = shell_page("<p>test</p>", &file, 0, 0).unwrap();
            assert!(shell.contains("Export filtered HTML"));
            assert!(shell.contains("window.ipc.postMessage('export-filtered')"));
            assert!(shell.contains(r#"sandbox="""#));
            assert!(!shell.contains("https://"));
        }

        #[test]
        fn original_office_diagnostic_is_explicit_and_sandboxed() {
            let names = vec![PathBuf::from("sample.eml")];
            let original = r#"<style>.xl1{border:1px solid black}</style><table><tr><td>test</td></tr></table>"#;
            let filtered = shell_page("<p>filtered</p>", &names, 0, 0).unwrap();
            let raw = shell_page(original, &names, 0, 2).unwrap();
            assert!(filtered.contains(r#"<option value="0" selected>Filtered HTML</option>"#));
            assert!(raw.contains(r#"<option value="2" selected>Original Office HTML (diagnostic)</option>"#));
            assert!(raw.contains(r#"sandbox="""#), "{raw}");
            assert!(!raw.contains("allow-scripts"));
            assert!(!raw.contains("allow-same-origin"));
            assert!(raw.contains("default-src 'none'"));
            assert!(raw.contains("border:1px solid black"));
            assert!(raw.contains("window.ipc.postMessage('mode:'"));
        }

        #[test]
        fn lottery_word_border_shorthands_and_spacers_survive_sanitizer() {
            let original = r#"<table style="width:352.7pt;margin-left:-.7pt;border-collapse:collapse"><tr>
              <td style="width:60.3pt;border:solid windowtext 1.0pt;border-top:none;padding:0cm 5.4pt 0cm 5.4pt">PP+Carry</td>
              <td style="width:99.7pt;border-top:none;border-bottom:solid windowtext 1.0pt;border-right:solid windowtext 1.0pt">13</td>
              <td style="width:23.0pt;padding:0cm 5.4pt 0cm 5.4pt"></td>
              <td style="width:53.0pt;border:solid windowtext 1.0pt;background:#FFFF99">1</td>
              </tr></table>"#;
            let filtered = safe_fragment(original);
            assert!(filtered.contains("border-collapse:collapse"), "{filtered}");
            assert!(filtered.contains("border:solid #000000 1.0pt"), "Word border shorthand lost: {filtered}");
            assert!(filtered.contains("border-bottom:solid #000000 1.0pt"), "Word side border lost: {filtered}");
            assert!(filtered.contains("background:#FFFF99") || filtered.contains("background:#ffff99"), "yellow background lost: {filtered}");
        }

        /// Excel 9 PCM uses a large .xl stylesheet and fixed table layout.
        /// Preserving its selectors should not require turning off CSP.
        #[test]
        fn pcm_excel_css_classes_and_fixed_layout_survive() {
            let html = r#"<style><!--
                .xl25 { border:.5pt solid windowtext; background:#CCFFFF; white-space:nowrap }
                .xl99 { border-left:none; border-top:none; text-align:center }
                --></style>
                <table style="border-collapse:collapse;table-layout:fixed;width:824pt">
                 <tr><td class="xl25" colspan="2">85人</td><td class="xl99">15:30</td></tr></table>"#;
            let css = safe_stylesheets(html);
            let filtered = safe_fragment(html);
            assert!(css.contains(".xl25"), "class selector stripped: {css}");
            assert!(css.contains("border:.5pt solid #000000"), "Excel border CSS stripped: {css}");
            assert!(css.contains("white-space:nowrap"), "Excel nowrap CSS stripped: {css}");
            assert!(filtered.contains("table-layout:fixed"), "Excel fixed layout stripped: {filtered}");
            assert!(filtered.contains(r#"colspan="2""#), "merged cell stripped: {filtered}");
        }

        #[test]
        fn converts_legacy_excel_windowtext_borders_without_stripping_table() {
            let src = r#"<table style="BORDER-COLLAPSE:collapse"><tr>
<td class="xl165" style="BORDER-RIGHT:windowtext .5pt solid; BORDER-BOTTOM:windowtext .5pt solid; BACKGROUND-COLOR:#ccffcc; WIDTH:47px">Item</td>
<td style="BORDER-LEFT:windowtext .5pt solid">Description</td></tr></table>"#;
            let normalized = normalize_office_inline_styles(src);
            assert!(normalized.contains("border-right:.5pt solid #000000"), "{normalized}");
            let sanitized = safe_fragment(src);
            assert!(sanitized.contains("<table"), "{sanitized}");
            assert!(sanitized.contains("border-right:"), "Office right border removed: {sanitized}");
            assert!(sanitized.contains("border-bottom:"), "Office lower border removed: {sanitized}");
            assert!(sanitized.contains("background-color:"), "Office fill removed: {sanitized}");
            assert!(sanitized.contains(r#"class="xl165""#), "Office class lost: {sanitized}");
        }

        #[test]
        fn keeps_office_class_styles_and_drops_remote_css() {
            let css = safe_stylesheets("<style>.xl165 { BORDER-RIGHT:windowtext .5pt solid; BACKGROUND-COLOR:#ccffcc }</style>");
            assert!(css.contains("border-right:") || css.contains("BORDER-RIGHT:"), "{css}");
            assert!(css.contains("#000000"), "{css}");
            assert!(safe_stylesheets("<style>.xl165 { background: URL(https://tracker.example/x) }</style>").is_empty());
        }

        #[test]
        fn preserves_nested_table_geometry_and_styles() {
            let html = r#"<style>.xl7 { background:#ccffcc; border:1px solid black }</style>
                <table width="853" cellspacing="0" style="border-collapse:collapse">
                <colgroup><col width="47"><col width="88"></colgroup>
                <thead><tr><th class="xl7">Name</th><th>Value</th></tr></thead>
                <tbody><tr><td colspan="2" rowspan="2" style="border:1px solid black">Merged</td></tr>
                <tr></tr></tbody></table>"#;
            let safe = safe_fragment(html);
            for tag in ["<table", "<colgroup", "<col", "<thead", "<tbody", "<tr", "<th", "<td"] {
                assert!(safe.contains(tag), "Lost {tag}: {safe}");
            }
            assert!(safe.contains("colspan=\"2\""), "Lost colspan: {safe}");
            assert!(safe.contains("rowspan=\"2\""), "Lost rowspan: {safe}");
            assert!(safe.contains("border-collapse"), "Lost inline border-collapse: {safe}");
        }

        #[test]
        fn strips_active_content_and_preserves_table_spans() {
            let s = safe_fragment("<table><tr><td colspan='2' rowspan='3' style='border:1px solid black' onclick='x()'>Hello</td></tr></table><script>alert(1)</script><img src='https://tracker.example/x'>");
            assert!(s.contains("colspan=\"2\""), "{s}");
            assert!(s.contains("rowspan=\"3\""), "{s}");
            assert!(!s.contains("onclick"));
            assert!(!s.contains("script"));
            assert!(!s.contains("tracker.example"));
        }

        #[test]
        fn html_file_selector_is_sandboxed_and_escapes_names() {
            let f = vec![PathBuf::from("A & \"B\".eml"), PathBuf::from(r#"C:\mailtests\second.eml"#)];
            let html = shell_page("<html><body>Test</body></html>", &f, 1, 0).unwrap();
            assert!(html.contains("sandbox=\"\""));
            assert!(html.contains("A &amp; &quot;B&quot;.eml"));
            assert!(html.contains("<option value=\"1\" selected>second.eml</option>"));
            assert!(html.contains("window.ipc.postMessage"));
            assert!(!html.contains("file:///"));
        }

        #[test]
        fn dropdown_selection_must_be_index_in_known_list() {
            assert_eq!(selected_index("select:1", 2), Some(1));
            assert_eq!(selected_index("select:2", 2), None);
            assert_eq!(selected_index("select:../../secret", 2), None);
        }

        #[test]
        fn navigation_allows_only_generated_html_data_urls() {
            let pages = ["<p>Known email</p>".to_string(), "<pre>Source</pre>".to_string()];
            let approved = [
                STANDARD.encode(pages[0].as_bytes()),
                STANDARD.encode(pages[1].as_bytes()),
            ];
            assert!(allowed_navigation("about:blank", &approved));
            assert!(allowed_navigation(&format!("data:text/html;base64,{}", approved[0]), &approved));
            assert!(allowed_navigation(&format!("data:text/html;charset=utf-8;base64,{}", approved[1]), &approved));
            assert!(!allowed_navigation("https://external.example", &approved));
            assert!(!allowed_navigation("file:///C:/secret", &approved));
            assert!(!allowed_navigation("data:text/html;base64,SGFja2Vk", &approved));
            assert!(!allowed_navigation("data:image/png;base64,SGFja2Vk", &approved));
        }

        #[test]
        fn office_comment_wrapped_css_keeps_safe_grid_rules() {
            let css = safe_stylesheets(
                "<style><!-- .xl65 { border:1px solid black;background-color:#ccffcc;width:90pt } --></style>"
            );
            assert!(css.contains(".xl65"), "{css}");
            assert!(css.contains("border:1px solid black"), "{css}");
            assert!(css.contains("width:90pt"), "{css}");
            assert!(safe_stylesheets("<style><!-- .xl65{background:url(https://tracker.example/x)} --></style>").is_empty());
            assert!(safe_stylesheets("<style>.bad{content:'</style><script>alert(1)</script>'}</style>").is_empty());
        }

        #[test]
        fn csp_and_office_css_are_networkless() {
            assert!(document("OK", "").contains("default-src 'none'"));
            assert!(safe_stylesheets("<style>.xl1{width:200pt;border:1px solid black}</style>").contains("width:200pt"));
            assert!(safe_stylesheets("<style>@import url(https://host)</style>").is_empty());
        }
    }
}
#[cfg(windows)]
fn main() {
    if let Err(error) = windows::run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
#[cfg(not(windows))]
fn main() {
    eprintln!("This WebView2 experiment is Windows-only.");
}
