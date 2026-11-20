# PROJECT_CONTEXT.md — MailCraft project context

Last reviewed: 2026-11-20 (UTC+08:00). Source hierarchy: GitHub code/branches/Actions > recorded reproducible tests > prior chat handoff. "待核實" means not proven.

## Repository and architecture
- Repository: https://github.com/colapig99/mailcraft (fork). Default integration baseline: `main`; verified baseline before these documentation commits: `aa81a1c91f9a469e11f1df56826fe94c9c1d61b9`.
- Original MailCraft is a Rust local-first mail client, with `apps/mailcraft`, `crates/engine`, `crates/net`, `crates/mime`, `crates/ui-egui`, `crates/store`, and a Cargo workspace. Read `README.md`, `AGENTS.md`, `docs/architecture.md` before changes.
- User's customization goals: legacy Exchange 5.5/2003 compatibility; IMAP, SMTP, LDAP GAL; efficient recipient search with low Exchange server impact; Chinese/English display names; local Contacts and Mail search; Public Folder / RSS visibility and folder tree; reliable HTML email tables, including Excel/Word HTML; Actions-produced Windows executables.
- **Distinguish product capabilities from deployed legacy Exchange compatibility.** The upstream README describes EWS and TLS-enabled services, but Exchange 5.5/2003 legacy interoperability is an ongoing fork-specific task, not assumed supported end-to-end.
- Legacy environment reported in prior conversations (not a live test): Exchange 5.5 at `qplchn08`; Exchange 2003 at `qplhkex01`; LDAP without TLS; IMAP/SMTP authentication differences; SMTP routing through Exchange sites. Do not enable legacy insecure networking without an explicit threat-model review. Actual connectivity and behavior: 待核實.
- Recipient-search history reports early aliases containing spaces (NOT commas) and later underscores, plus exceptional formats. English plus optional Chinese display name is desired. Query semantics, permissions, rate limits and actual full GAL coverage: 待核實.
- Local UI/custom functionality appears on multiple isolated branches; presence of a branch is NOT proof of a feature in `main`.

## HTML rendering — verified design boundary
- Native application HTML reader uses Rust MIME/layout and egui pathways, including `crates/mime/src/html/`, `crates/mime/src/layout/`, `crates/ui-egui/src/reading/body/`.
- Isolated WebView2 experiment lives at `poc/webview2/`; source and build workflow `.github/workflows/webview2-poc-windows.yml`. It is **outside the Cargo workspace and NOT an integrated MailCraft reader or replacement EXE**.
- As documented in `poc/webview2/README.md` at `bc543382569c11f0428359e8586c99720ceb775f`: default filtered HTML must stay unchanged; experimental Office mode opt-in only. Excel-specific transformation is gated on **both** numeric `x:num` cells and `mso-spacerun` markers. Non-Excel Word/lottery, Veeam and generic HTML receive the unchanged sanitized direct document in Office mode. Conservative detection may miss Excel HTML that lacks either marker.
- Diagnostic V2 compares TWO SANITIZED paths at matched viewport, not a raw-source vs sanitized DOM proof. Reports exclude email body text but can contain sensitive layout metadata. The unsanitized Original Office preview is experimental, not approved for production mail.
- Production integration requires isolation/security review, WebView2 lifecycle, focus/DPI, CID, links, attachments, print, fallback, and actual Windows visual regression evidence. See POC README for full constraints.
- Never change table widths/borders/fonts globally to fix one EML. Keep fixes source-scoped and evidence-led.

## Protected behavior / non-regression
- Existing IMAP/SMTP, mail sync/store, LDAP/GAL, contact search, folder and Public Folder selection, RSS visibility, and native renderer must not regress.
- Confirm behavior on each branch using its code and tests. Do not assume changes in `feature/manual-public-folder-selection`, `feature/rss-feeds-visibility`, or `feature/exchange-legacy-ldap` are merged.
- Do not commit real customer EMLs, mailbox data, credentials, or unsanitized HTML exports. Keep fixture data synthetic or redacted.

## Branch and merge policy
1. `main` is the stable baseline until an explicit verified integration changes that policy. Each feature uses its own branch. Known branches as of review: `feature/html-webview2-poc`, `feature/html-table-rendering`, `feature/exchange-legacy-ldap`, `feature/manual-public-folder-selection`, `feature/public-folder-settings-panel`, `feature/rss-feeds-visibility`, and multiple `fix/*` branches. Re-list branches on every handoff.
2. Before edits, fetch branch head, latest commits, related PRs and Actions. Compare with `main` and other relevant feature branches; never overwrite parallel work.
3. Test on the branch, check changes and regression scope, merge via reviewable PR only when ready; retain branch/commit provenance. Avoid assuming a green build equals visual approval.
4. Keep `PROJECT_CONTEXT.md` relatively stable; update `DEVELOPMENT_STATUS.md` and `TEST_RESULTS.md` after important edits/tests. Documentation commits may advance `main` HEAD without changing app behavior.
5. At next conversation: read all three root documents, recheck GitHub refs/builds, then resume. If unexpected truncation prevented docs updates, recover state from commits/Actions, and mark unverified items.
