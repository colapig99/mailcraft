# WebView2 in-memory preview integration (Windows, experimental)

Status: **feature branch only**, not merged to `main`, and not an embedded egui reading-pane renderer. This is the first *end-to-end integration step*: open an HTML message already loaded in MailCraft with the standalone WebView2 viewer, without saving an intermediate EML.

## Scope / source paths

- UI opt-in item: `crates/ui-egui/src/reading.rs` → More actions → **Preview in WebView2 (experimental)**. Only shown on Windows for unprotected HTML messages.
- UI intent: `Action::PreviewWebView2` (`crates/ui-egui/src/lib.rs`) and dispatch in `crates/ui-egui/src/app/actions.rs`. Uses `Session::message_source()` so it reads the already cached original MIME body; never changes IMAP, SMTP, LDAP or folder synchronization.
- Platform contract: `Services::preview_webview2(&[u8])` with an unsupported default for non-Windows consumers. Windows implementation: `apps/mailcraft/src/desktop.rs`.
- Companion EXE: `poc/webview2/src/main.rs` with private `--stdin` input, at most **12 MiB**. The selected MIME input arrives over an anonymous pipe and is parsed/sanitized in-memory. No credentials, source HTML, message keys or EML paths go into process arguments.
- Browser safety: the child view retains existing sandbox, CSP and navigation restrictions; live mode disables original unsanitized Office preview, layout diagnostics, file browsing, and HTML exports. The normal native egui reader remains the default.
- Build workflow: `.github/workflows/webview2-integrated-windows.yml`. It tests the POC and native UI, builds both Windows EXEs and places them side-by-side in one artifact.

## Current limitations / release gate

- This is **a separate, read-only viewer window**. It does not yet embed WebView2 inside the MailCraft reading pane, track egui scroll position, preserve CID inline images, manage mouse/focus/DPI together with egui, or implement printing, attachment/link commands, and WebView2 lifecycle in the main process.
- The sanitization/CSS gate is still experimental and has NOT undergone production security review. Do not set WebView2 as the automatic/default renderer.
- No raw email is saved by the handoff, but the companion process holds the message in memory. The original MailCraft cache remains unchanged.
- Only mark this branch visually accepted after Windows smoke testing; passing Rust tests and Actions build alone proves compilation/test assertions, not correct pixel layout in every message.
- Before merging, compare with `feature/html-table-rendering` (diverged) and all legacy LDAP, Public Folder, and RSS branches; verify no missing modifications. The integration branch was started at `98959c23864b4756d76e48f431e1b799a7d6a3b8`.

## Minimal manual test after successful Windows Actions build

1. Download `mailcraft-webview2-integration-windows`, extract and **leave the two EXEs side-by-side**.
2. Run `mailcraft.exe`, open a normal HTML message (or PCM test in a local sample account if available).
3. Use **More actions → Preview in WebView2 (experimental)**. A separate WebView2 window should show the same message; no new .eml file is written.
4. Confirm the native reader remains usable. Compare one PCM/lottery HTML display against the existing baseline if those samples are present in this MailCraft account.
5. Verify missing helper EXE reports a clear notice, signed/encrypted messages do not offer the menu, and remote images/scripts do not execute.
6. Do NOT test a production mailbox if the separate security review is a requirement. Use sanitized synthetic content first.

## Next integration milestone (not implemented)

- Replace process handoff with a Windows child-WebView lifetime managed by the native eframe/winit host and feed an approved sanitized in-memory HTML document.
- Implement placement over an egui-allocated region, show/hide with selection, resize/DPI/focus sync, CID content controls, explicit link/print handling, and precise security tests.
- Keep egui as a per-message fallback, avoid creating a WebView per row, and guard resource usage.

## Build efficiency change

At commit `e6626444e0c5aed3ab6ac852613d8311c6f73678`, the Windows integration workflow was split into **independent parallel jobs**:
- `windows-exes`: build release MailCraft, test/compile sandboxed WebView2, and upload both EXEs side-by-side as one artifact.
- `ui-regression`: execute egui UI tests separately.

This avoids waiting for the full UI regression job before an EXE artifact can be produced. **Do not call the artifact stable unless both jobs pass and Windows manual integration tests have been recorded.** Build #1 used the old sequential workflow; #2 is the first parallel version. Both still require successful runner execution; no artifact is assumed until actually observed.
