//! Standalone, Windows-only WebView2 rendering experiment.
//! Usage: mailcraft-webview2-poc.exe path-to-message.eml
//! F1: Chromium HTML view; F2: sanitized HTML source; Esc: exit.
//! NOT YET embedded in MailCraft. Never connects to Exchange or changes mail.
#[cfg(windows)]
mod windows {
    use std::{collections::HashSet, fs, path::PathBuf};
    use mail_parser::MessageParser;
    use winit::{
        application::ApplicationHandler,
        event::{ElementState, WindowEvent},
        event_loop::{ActiveEventLoop, EventLoop},
        keyboard::{KeyCode, PhysicalKey},
        window::{Window, WindowId},
    };
    use wry::{WebView, WebViewBuilder, Rect};

    const MAX_EML_BYTES: u64 = 12 * 1024 * 1024;
    const MAX_HTML_BYTES: usize = 1_500_000;
    const CSP: &str = "default-src 'none'; script-src 'none'; connect-src 'none'; img-src data:; style-src 'unsafe-inline'; font-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'";
    const HELP: &str = "<div style='position:sticky;top:0;background:#edf3ff;border-bottom:1px solid #aaa;padding:7px;font:13px Segoe UI,sans-serif;z-index:1000'>WebView2 comparison POC — F1: formatted HTML / F2: sanitized source / Esc: close. No network images or scripts.</div>";

    fn escape_html(input: &str) -> String {
        input.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
    }

    /// Do not expose raw untrusted HTML to a browser. Preserve safe table
    /// geometry and a strict subset of inline CSS for the compatibility test.
    fn safe_fragment(html: &str) -> String {
        let mut cleaner = ammonia::Builder::default();
        cleaner
            .rm_tags(&["img", "form", "input", "button", "svg", "video", "audio"])
            .add_generic_attributes(&[
                "style", "class", "width", "height", "align", "valign",
                "bgcolor", "cellpadding", "cellspacing", "border",
                "colspan", "rowspan", "nowrap",
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
        cleaner.clean(html).to_string()
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
            let css = cap.get(1).map_or("", |m| m.as_str());
            if css.len() > 128 * 1024 || out.len() + css.len() > 128 * 1024 {
                break;
            }
            let lower = css.to_ascii_lowercase();
            if css.contains('<')
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

    pub(crate) fn pages(raw: &[u8]) -> Result<[String; 2], String> {
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
        let inspect = document(&format!(
            "<h3>Sanitized HTML generated by the POC</h3><pre style='white-space:pre-wrap'>{}</pre>",
            escape_html(&safe)
        ), "");
        if formatted.len().max(inspect.len()) > 2 * 1024 * 1024 {
            return Err("Result exceeds WebView2 2 MiB HTML string limit".into());
        }
        Ok([formatted, inspect])
    }

    struct Viewer {
        window: Option<Window>,
        webview: Option<WebView>,
        views: [String; 2],
        mode: usize,
    }

    impl ApplicationHandler for Viewer {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            if self.window.is_some() {
                return;
            }
            let attrs = Window::default_attributes()
                .with_title("MailCraft HTML WebView2 POC — F1 HTML / F2 Source")
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
            let builder = WebViewBuilder::new()
                .with_bounds(bounds)
                .with_html(self.views[self.mode].clone())
                .with_javascript_disabled()
                .with_devtools(false)
                .with_incognito(true)
                .with_general_autofill_enabled(false)
                .with_on_page_load_handler(|event, url| eprintln!("WebView2 page event: {event:?} URL: {url}"))
                .with_navigation_handler(|url| {
                    // NavigateToString is about:blank in WebView2. Log all
                    // navigation attempts and reject unexpected destinations.
                    let ok = url == "about:blank";
                    eprintln!("WebView2 navigation: {url} allowed={ok}");
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
                        PhysicalKey::Code(KeyCode::F1) | PhysicalKey::Code(KeyCode::F2) => {
                            let next = if event.physical_key == PhysicalKey::Code(KeyCode::F1) { 0 } else { 1 };
                            if next != self.mode {
                                self.mode = next;
                                if let Some(webview) = self.webview.as_ref() {
                                    if let Err(e) = webview.load_html(&self.views[next]) {
                                        eprintln!("Unable to change display mode: {e}");
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
        let mut args = std::env::args_os().skip(1);
        let path = args.next().map(PathBuf::from).ok_or(
            "Usage: mailcraft-webview2-poc.exe path-to-message.eml (F1 HTML, F2 source)"
        )?;
        let meta = fs::metadata(&path).map_err(|e| format!("Cannot read EML: {e}"))?;
        if meta.len() > MAX_EML_BYTES {
            return Err("EML exceeds 12 MiB POC limit".into());
        }
        let raw = fs::read(path).map_err(|e| format!("Cannot open EML: {e}"))?;
        let views = pages(&raw)?;
        let event_loop = EventLoop::new().map_err(|e| format!("Window event loop failed: {e}"))?;
        event_loop.run_app(&mut Viewer { window: None, webview: None, views, mode: 0 })
            .map_err(|e| format!("WebView2 event loop failed: {e}"))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

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
