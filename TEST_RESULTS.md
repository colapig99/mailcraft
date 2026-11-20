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

## 2026-11-20 — User-provided Windows screenshots of Build #46 (manual visual evidence; partial)

Evidence: six screenshots supplied in ChatGPT conversation after run #46 download. Screenshots themselves are **not checked into this public repository**; source EMLs remain private. Assumed tested artifact: downloaded #46 POC per preceding conversation; executable checksum, Windows edition, scale and WebView2 runtime: 待核實. Source code branch: `feature/html-webview2-poc`, `bc543382569c11f0428359e8586c99720ceb775f`.

1. **PCM overtime, paired screenshots:** In `Filtered HTML`, several numeric values visibly intrude/overlap with adjacent cell content (e.g. first table overtime totals and monthly accumulated hours). In `Office compatibility (experimental)`, comparable rows are visibly separated and legible, including `19.5`, `39.0`, `208.0`, and monthly totals. **Manual visual: improvement supported; conditional PASS for the overlapping-numbers symptom**, not a blanket PASS for all Excel layout fidelity. Table column widths and typography/layout differ visibly between modes; exact Outlook-reference fidelity not assessed. Same viewport/DPI: 待核實.
2. **Lottery issue #3, paired screenshots:** `Filtered HTML` and `Office compatibility` captures have different visible content/placement: the Office capture shows a `Subject:` line, more quoted mail and footer while the filtered capture starts closer to the lottery table. The table itself appears broadly similar, without obvious new font shrinkage in the visible cells, but screenshots are NOT aligned to a common viewport/scroll position. **Manual visual verdict: 待核實; cannot prove text/body preservation or pixel equivalence.** Determine if different content is due to scroll position, variant selection, message processing or another factor before considering any fix.
3. **Lottery issue #1:** Two images both have `Office compatibility (experimental)` selected; the captures look effectively identical. This is **not** a filtered-vs-Office comparison, so **regression verdict: 待核實**.
4. **General normal HTML:** No clearly identified paired normal-HTML control screenshots supplied: **待核實**.
5. CI #46 succeeded (automated); no production reader integration demonstrated. No code modifications made based on these screenshots.

### Focused next validation
- Repeat lottery issue #3 at the same window dimensions and scroll position; use **Filtered HTML** then **Office compatibility** for the same EML and take whole-page/matching-region captures. Verify quoted body, subject header, fonts and border/width fidelity. If content truly differs, inspect exact sanitized HTML; do not infer that the Excel gate is to blame.
- Repeat lottery issue #1 in both different modes (the current two screenshots show the same Office mode).
- Run one normal HTML control in both modes.
- Keep PCM as a documented positive partial fix; inspect any shifted column geometry against Outlook 2007 original before signing off.

## 2026-11-20 16:15 UTC+08 — Additional Build #46 user screenshots (four images)

Context: user supplied a new paired capture of lottery issue #3 and two Veeam backup notification views. No customer EMLs or screenshots are committed to this public repository. Tested POC branch is still `bc543382569c11f0428359e8586c99720ceb775f`; local binary hash / WebView2 version / DPI: 待核實.

- **Lottery #3, Filtered HTML vs Office compatibility:** Both screenshots show the same lottery table, red numbers, black cell grid, yellow ranking cells, and the yellow PP/Sang/Carry comparison strip. No obvious within-table font-shrink regression or newly missing borders in the displayed region. However, the Filtered capture visibly includes a quoted `From / Sent / To / Cc / Subject` header and the preceding `Thanks and Best regards` text above the table; the Office view instead begins at `Subject:` and has a different visible position/layout. The modes were selected correctly, but the visible screenshots differ in window size/crop and page position, so **full-content parity remains 待核實**, not a definitive bug diagnosis. Preserve the distinction between the table regression (visually promising) and body content/scroll differences (unresolved).
- **Veeam backup notification:** Two views show the same main green Success header, backup details, numeric values and bordered table structure, including `43.1 MB`, `2.4 MB`, `18.09x`, catalog sizes and processing counts. No obvious overlapping numbers or missing table boundaries. One image is cropped without the mode selector, so its exact display mode cannot be independently proven from the screenshot; the other clearly shows Filtered HTML. Differences in table width and viewport/crop may be visual framing, not rendering regression. **Manual regression: partial visual evidence only; conditional table-layout PASS, cross-mode parity 待核實.**
- **Conclusion:** Keep Excel-specific Office rendering restricted; do not add another global CSS fix based on these images. Next objective check: obtain a matched viewport/scroll comparison or use locally exported **filtered HTML** for lottery #3 to establish whether body differences are due to scroll/cropping, source selection, or transformation. A complete regression matrix is still pending. CI status remains success for #46; Windows production integration not established.
