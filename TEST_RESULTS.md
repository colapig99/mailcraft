# TEST_RESULTS.md — objective testing and evidence

Snapshot: 2026-11-20 (UTC+08:00). **Never equate GitHub CI success with human Windows screenshot acceptance.** Use 待核實 when evidence is absent.

## Confirmed GitHub Actions observations
| Run | Branch / commit | Outcome | Confirmed scope |
| --- | --- | --- | --- |
| WebView2 POC Windows #46 | `feature/html-webview2-poc` / `bc543382569c11f0428359e8586c99720ceb775f` | **success**, completed | Job steps: security and HTML layout tests, build standalone comparison tool, upload artifact all success. Run id `44681306033`. |
| WebView2 POC Windows #45 | `58dfcaaa6f8b0779e6a1331e4bc5f8a7be3c3943` | **success**, completed | Previous regression-focused commit; detailed test assertions: inspect workflow/logs if needed. |
| WebView2 POC Windows #44 | `55722092af6f7c7204b8689c27be02c194c54125` | **failure**, completed | Cause: 待核實; superseded by later green builds but not silently ignored. |

#46: https://github.com/colapig99/mailcraft/actions/runs/44681306033

Artifact observed: `mailcraft-webview2-poc-windows`; upload step success. This is NOT evidence that production `mailcraft.exe` was built or that WebView2 is integrated into the reading pane.

## Manual Windows acceptance matrix (NOT yet verified for #46)
| Sample | Source / intent | Method | Expected | Observed on #46 | Verdict |
| --- | --- | --- | --- | --- | --- |
| `PCM部加班申请---今天 TEST.eml` | Previous chat: numeric overlap and merged cells | Same size/DPI: Filtered HTML vs Office compatibility; optionally V2 sanitized comparison | Numbers not overlapping; no unrelated font/border/width changes | 待核實 | 待核實 |
| `2026 十期六合彩 (第1期).eml` | Previous chat: Word HTML shrank fonts after broad Office patch | Same conditions; verify Office gate skipped for non-Excel | Fonts, borders, table widths unchanged against standard direct-filtered path | 待核實 | 待核實 |
| Normal HTML mail | Generic control | Both display modes; record side-by-side | No change in type, width, flow, links | 待核實 | 待核實 |
| MIS purchase / weekend roster | Cross-table regression | See POC README | Preserve grid and horizontal/vertical boundaries | 待核實 | 待核實 |
| Veeam / Thunderbird / Gmail | Non-Excel control | See POC README | Preserve nowrap, row heights, nested/borderless tables | 待核實 | 待核實 |

## Prior conversation evidence (reported, NOT a fresh re-test)
- User previously reported that an Office display-mode switch corrected a problematic table yet made other table fonts smaller; screenshots existed in the prior chat. No proof this persists in #46.
- Prior discussion used `.layout-01.json` and `.layout-02.json` for PCM and lottery. Exact JSON figures were not independently re-examined in this snapshot. Any numeric claim must be backed by those files and relevant commit/build.
- Reported PCM numeric overlap improvement and proposed Excel-only gating motivated commits `55722092`, `58dfcaaa`, `bc543382`. Git commit and #46 CI evidence establishes implementation/build, not visual acceptance.

## Reproducible manual check protocol
1. Download the artifact for run #46 from GitHub Actions, note checksum/version, and extract it separately from production MailCraft. Keep source EMLs LOCAL; do not push customer messages.
2. Put the three EML samples next to the standalone POC EXE. Record Windows version, WebView2 Runtime version, monitor scaling, window viewport, sample hash and timestamp.
3. For each sample, run standard Filtered HTML and experimental Office compatibility at the SAME viewport/DPI and capture before/after screenshots. Verify the selected display mode remains fixed when changing EMLs.
4. Use V2 comparison report as evidence for measured geometry computed between two sanitized variants. Remember this does not establish raw-vs-sanitized provenance.
5. Log cell overlap count, widths, font size, borders, row/col spans, clipping, and any unexpected behavior; link the exact commit, branch, Action run, fixture/hash, and screenshots.
6. Mark each separately: (a) automated tests PASS/FAIL, (b) Windows visual PASS/FAIL/待核實, (c) production integration PASS/FAIL/待核實.
7. On regressions, inspect original Office HTML versus filtered HTML and sanitization rules before changing CSS. Add synthetic regression coverage and rerun Windows comparison.

## Security and privacy
Sanitized diagnostic JSON may reveal table structure; inspect before sharing. Source EMLs and exported filtered HTML may contain customer data. Never commit confidential samples or credentials.

## Test record template
- Date/time/timezone:
- Machine / OS / scaling / WebView2 runtime:
- Repository / branch / exact commit:
- Actions run / artifacts:
- Fixture filename + SHA256 (do not commit private mail):
- Reproduction steps and expected result:
- Measured results + screenshot/report references:
- Automated CI verdict:
- Manual Windows visual verdict:
- Regression verdict:
- Root cause, if verified:
- Next action / owner:
