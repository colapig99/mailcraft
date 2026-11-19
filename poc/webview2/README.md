# MailCraft WebView2 HTML Renderer — isolated POC

**Status: experiment, NOT the default renderer and NOT an integrated egui mode.**

This directory is deliberately outside the Cargo workspace. The production
MailCraft app, its native egui renderer, legacy Exchange integration, IMAP,
LDAP, Public Folders, RSS Feeds and Store are untouched.

## Build (Windows 10/11)

Microsoft Edge WebView2 Runtime must be available.

```powershell
cargo test --manifest-path poc/webview2/Cargo.toml
cargo build --manifest-path poc/webview2/Cargo.toml --release
.\poc\webview2\target\release\mailcraft-webview2-poc.exe "C:\mail-tests\sample.eml"
```

## Choose an EML from the EXE folder

Place your test `.eml` files in the same folder as
`mailcraft-webview2-poc.exe`. Double-click the EXE or run it without
arguments: it scans that EXE folder for up to 500 `.eml` files, sorts them by
name, and selects the first one. The toolbar at the top includes an **EML file**
dropdown and **Refresh list** button. Choosing another file switches its
preview without closing the program. Refresh rescans after copying new files
into the folder.

A filename argument is still accepted to choose an initial EML **by basename
from that same EXE folder**; it does not grant access to other folders.

POC keyboard controls: **F1** for formatted WebView2 HTML, **F2** for its
sanitized HTML source, **Esc** to close. The in-email checkbox can also switch
between HTML layout and sanitized markup. The standalone tool can run next to
an unchanged MailCraft installation for manual comparison. The direct
in-reading-pane egui/WebView2 toggle remains a separate integration milestone.

The toolbar is a trusted local UI and uses Wry IPC only for a numeric file
index or Refresh. The HTML email is placed in a sandboxed iframe without script,
same-origin or form permissions, even though a small toolbar-only script is
enabled for dropdown interaction.

## Export the sanitized HTML for targeted investigation

The **Export filtered HTML** button writes the currently selected EML's
sanitized preview into the EXE folder as
`<original filename>.eml.filtered.html`. The source EML is unchanged.
The export is local and requires a click; no data is uploaded to GitHub,
MailCraft or a remote service. An existing export is **not overwritten**.

This diagnostic file contains the email content, including potentially
confidential information. Review it before sharing. It lets us compare the
exact HTML/CSS that WebView2 receives against the original EML, rather than
guessing from screenshots. Do not treat this POC-exported HTML as a
production-certified sanitizer output.

## Office HTML comparison mode

Choose **Display → Filtered HTML** for the existing safe default, **Filtered
HTML source** for the resulting markup, or **Original Office HTML
(diagnostic)** to compare the sender's original layout without HTML
sanitization. This option exists **only in this standalone local POC**.

The original-content preview stays inside an iframe with an empty
`sandbox` attribute. Its parent page applies CSP restrictions to scripts,
remote resources, frames and navigation. This is an experimental diagnostic
feature, **not a production-safe authorization to render arbitrary mailbox
HTML**. Avoid opening files from untrusted sources in this mode.

A fourth option, **Direct filtered document (experimental)**, removes the
additional comparison wrappers around the sanitized HTML. It is useful for
checking whether those wrappers interfere with legacy Excel fixed-width and
merged-cell layout. The default display mode remains unchanged, and the
original sender's script/remote-resource restrictions remain in force.

If the original HTML looks correct but the filtered version does not, focus
on HTML/CSS sanitization. If both exhibit the same error, investigate
Chromium compatibility with Word/Excel HTML and source markup.

## Office compatibility opt-in experiment

**Display → Office compatibility (experimental)** keeps safe Excel
`<font>` runs during sanitization and reconciles only Word-labelled
`MsoNormalTable` width attributes when the HTML pixel width is more
than 10% larger than the CSS point-based width. It does not synthesize
missing cell borders. The default **Filtered HTML** stays unchanged.

Compare the PCM (first table numerical overlap) and lottery (Word-table
widths and border gaps) EMLs in both modes. Check existing MIS purchase,
MIS weekend and Veeam emails for regression in default mode before adopting
any change. Original Office HTML remains an isolated diagnostic only.

## Manual regression matrix (do not commit real customer EMLs)

1. PCM overtime: yellow merged headers, blue headcount cells, historical months.
2. MIS purchase: 18 columns, all vertical/horizontal boundaries, horizontal scrolling.
3. MIS weekend duty: 7-column roster, uninterrupted vertical borders.
4. Lottery: ranking spacer, merged headers and fixed widths.
5. Veeam: percent/nowrap status, variable row heights and trailing rowspan spacer.
6. Thunderbird/Gmail: borderless and nested layout tables.
7. Validate both reader pane and separate window **after** integrating.

Compare text wrapping, fixed/percent widths, colspan/rowspan, colors, fonts,
scrolling, time to first draw, memory and CPU. The standalone POC itself
cannot prove the native egui pane and WebView2 child overlay are integrated.

## Security limits

The POC parses EML locally, uses an HTML5 allowlist sanitizer with a restricted
inline CSS property set, discards remote images/forms/scripts, and loads only
in-memory content. It sets a restrictive CSP, disables JavaScript, blocks
navigation, turns off autofill and uses an incognito WebView. CSS <style> rules
are copied only through a basic conservative network/markup gate. **That gate
is experimental, NOT a production-grade CSS sanitizer**; expand security
testing before connecting the viewer directly to an untrusted mailbox.

The HTML-string WebView2 method has a 2 MiB cap, so very large EMLs are rejected
instead of silently truncated. CID images, print, context menus, links,
attachments, tracking-pixel preferences, WebView2 process lifetime, DPI and
parent-viewport focus must be implemented and tested before shipping.

## Architectural decision

Prefer a Windows child WebView2 owned by the host winit/eframe window, with
a per-message renderer preference and a fall back to the unmodified egui
renderer. Never create a WebView for every message row. Integrate only after
this standalone POC passes actual screenshot comparison.
