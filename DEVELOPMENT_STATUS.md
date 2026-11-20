# DEVELOPMENT_STATUS.md — MailCraft development snapshot

Last reviewed: 2026-11-20 (UTC+08:00). Document is a point-in-time verified snapshot; **refresh GitHub refs on every new conversation**.

## GitHub baseline at review (BEFORE documentation-only commits)
- Repo: `colapig99/mailcraft`; default `main` = `aa81a1c91f9a469e11f1df56826fe94c9c1d61b9` (upstream-oriented baseline, last code commit seen 2026-11-08).
- Active HTML POC branch `feature/html-webview2-poc` = `bc543382569c11f0428359e8586c99720ceb775f`, 365 commits ahead of `main`, 0 behind, according to GitHub compare at review. This is not merged into main.
- Additional branch heads at review:
  - `feature/html-table-rendering` = `543bc6c366495661918fe0163e10dbd88c95c808`
  - `feature/exchange-legacy-ldap` = `7c11aa062c91ca826fb561a3f5ac75e7af0fd655`
  - `feature/manual-public-folder-selection` = `3275b635888280f280c0398d27ebc11fecda2347`
  - `feature/public-folder-settings-panel` = `3837692019ba7a826239155744c07e732c85fc0d`
  - `feature/rss-feeds-visibility` = `7e1a81c98b441c0c00259976b1a1e1ac5718633e`
  - `fix/public-folder-tree-and-lock-test` = `ebf5674d17a16252b6463f193a1dd976e6f36d29`
  - Other diagnostic/fix branches exist: enumerate again before integration.
- Open PR observed: #1, WIP LDAP GAL support, `feature/exchange-legacy-ldap` → `main`; not treated as merged.
- Documentation commit SHA(s) that create/update these three handoff docs: see Git history for `PROJECT_CONTEXT.md`, `DEVELOPMENT_STATUS.md`, `TEST_RESULTS.md`. This snapshot intentionally records pre-doc code refs to avoid circular SHA claims.

## HTML POC work, verified in Git
- Recent commits on `feature/html-webview2-poc`:
  - `1737e612...`: opt-in numeric Excel spacer removal.
  - `12483a61...`: sanitized paired layout V2 diagnostic.
  - `a3993edf...`: preserve chosen display mode when switching EML.
  - `55722092...`: scope experimental Excel formatting to confirmed cells.
  - `58dfcaaa...`: correct Excel detection regex; add MIME fixture regression tests.
  - `bc543382...`: document Excel-only guard and lottery-font regression procedure.
- Windows WebView2 POC workflow **run #46**, run id `44681306033`, status completed, conclusion success, commit `bc543382...`. Workflow job reports successful security/layout test step, standalone Windows build and artifact upload. Artifact: `mailcraft-webview2-poc-windows` (was not expired at review). Link: https://github.com/colapig99/mailcraft/actions/runs/44681306033 .
- This produces a **standalone comparison POC**, NOT the production MailCraft Windows EXE. Native egui/WebView2 integration and real Windows pixel/visual acceptance remain **待核實 / not completed**.
- Build #45 success; #44 failure. Root causes for earlier failed runs: 待核實; do not generalize without logs.

## Other modules / status
- `main` contains Rust native MailCraft; its own project README lists IMAP/POP/SMTP, searching, contacts, folders, and native reading, but exact fork-specific legacy compatibility needs branch-level verification.
- Exchange LDAP GAL, Public Folder / RSS visibility and folder safety work exist in named branches. They must be audited for differences and compatibility before any merge. End-to-end user acceptance and merge status: 待核實.
- Earlier conversation reports Public Folder/RSS UI behavior satisfactory in a separate branch. This is reported feedback, not proof of inclusion in `main`.

## Current priorities
P0. Re-run Windows POC #46 artifact on real EMLs at controlled viewport and DPI: PCM overtime, lottery, and generic normal HTML. Compare filtered / experimental Office; record screenshots, counts, font sizes, borders, column widths, and success/failure. If sensitive, keep EMLs local.
P0. Verify the Excel-specific gate is not applied to Word lottery/generic mail; confirm PCM improvement and no unwanted shrinkage on Windows. Git tests are not sufficient proof.
P1. If failure remains, compare exact sanitized HTML and source markup/DOM/CSS; use existing V2 diagnostics and isolate cause before any CSS change.
P1. Audit multiple feature branches and identify desired merge base; verify no public-folder/RSS/GAL functionality is dropped. Merge only after regression tests.
P2. Plan production rendering integration only after POC passes visual+security acceptance.

## Handoff checklist
At each significant test/commit: write actual branch + SHA, Action run + job conclusion, local Windows evidence, open defects, and next step into status/test docs. If interrupted, rebuild snapshot from GitHub rather than assuming the previous chat completed.

## 2026-11-20 — Fast diagnostic iteration (after Build #46)

- User requested fewer repetitive test rounds and a general HTML rendering approach rather than per-table CSS tweaks.
- Source review verified existing POC `Export filtered HTML` always writes `views[3]`, regardless of selected display mode. For non-Excel mail `views[3] == views[4]` is an intended code invariant; main Filtered display mode 0 adds a source-toggle wrapper, so visible scroll/vertical positioning can differ from Office mode without a changed sanitized email document.
- On `feature/html-webview2-poc`, commit `d4736377fde8a764e392fa46740db4e10f96010e` adds **Export both modes**: one explicit click writes a matched sanitized direct HTML and Office experimental HTML from the same EML, without overwrite. Adds test asserting Word/non-Excel input equality. Commit `98959c23864b4756d76e48f431e1b799a7d6a3b8` documents workflow/privacy.
- Windows Actions runs #47 (code) and #48 (documentation branch head) were **in_progress when checked**, not yet verified passing. Full build/test result: 待核實. Official app not changed or integrated.
- No additional CSS or renderer modifications made. Next user-visible test: after a green Windows build, launch POC once, select lottery #3 and PCM, click **Export both modes** once for each. Compare file hashes/structural differences before making any further rendering changes; never upload unredacted customer mail to GitHub.

## 2026-11-20 — Paired-HTML investigation: decisive evidence

- Windows POC Actions #47 and #48: **completed/success**; #48 branch head `98959c23864b4756d76e48f431e1b799a7d6a3b8`. This remains an isolated POC; formal reader integration not completed.
- User uploaded 8 exported HTML files. Local byte-level comparison: both lottery issues (1 and 3) have **identical** Direct and Office sanitized HTML (SHA-256 verified; respectively 230 and 464 TD cells). No Word/lottery table changes from the Excel-specific Office path; prior screenshot vertical differences are not evidence of sanitized-body loss.
- PCM overtime: Direct and Office differ in 30 TD inner HTMLs, removing spacer-only whitespace, while normalized cell text, all 307 TD attributes and stylesheets match. Prior Windows screenshots support an overlap improvement. This is a **scoped** Excel correction, not evidence of universal Office compatibility.
- MIS-weekend `1）.eml` had two identical Office exports but no Direct counterpart; do not claim paired test PASS. Full metrics and hashes are in `TEST_RESULTS.md` under 2026-11-20 paired HTML investigation.
- Next engineering priority: general WebView2 integration/security architecture, avoiding per-email CSS hacks. Evaluate trusted host + sandboxed message surface + safe CSS preservation + low-cost fallback. No new CSS edits justified by this batch. Distinguish POC success from product integration and real manual cross-client signoff.
