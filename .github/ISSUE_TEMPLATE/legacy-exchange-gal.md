---
name: Legacy Exchange GAL support / 遺留 Exchange GAL 技術支援
about: Report an anonymized issue with the experimental LDAP GAL connector
title: "[Legacy GAL] "
labels: []
assignees: []
---

> **Sensitive data warning / 敏感資料**: Do **not** include real email addresses, employee names, internal IPs/hostnames, Base DNs, bind identities, passwords, mail bodies, complete `accounts.json`, `mailcraft.log`, or backups. Keep those in an approved internal support ticket. / 請勿公開真實員工資料、內網主機／IP、憑證或完整日誌。

## Build and environment / 建置及環境
- Git commit SHA:
- Windows test artifact name:
- GitHub Actions Run URL:
- ZIP SHA-256:
- Windows version (general version only):
- Domain-joined client? Yes / No (no actual domain name):
- Data mode: normal / portable / custom --data:
- LDAP setting: Windows Integrated / TLS / anonymous (no endpoint or Bind DN):
- Directory schema: Exchange 2003 AD / Exchange 5.5 / other:

## Symptom / 症狀
- Category: installation / account settings / GAL only / suggestion source / performance / IMAP / SMTP / unrelated
- Expected:
- Actual:
- When did it start?
- Does the same person search work using the read-only Windows LDAP probe? Success / 0 results / error / not tested (no real names):

## Reproduction / 重現方法
1.
2.
3.

## Safe diagnostics / 可公開的診斷
- Sanitized error category / ResultCode:
- Returned result count only:
- Lookup length (characters, not actual name):
- Lookup elapsed time (ms):
- Does the second same lookup come from cache?
- PowerShell process count before/after lookup:
- Additional sanitized observations:

## Impact, workaround, rollback / 影響及回復
- Number of test users (5 / 20 / 50 / other):
- Workaround:
- Rollback tested? Yes / No:
- PDC CPU/RAM changes (aggregated only, no server identifiers):
- Does IMAP/SMTP still work?

## Verification
- [ ] I checked the correct artifact/commit rather than an older ZIP.
- [ ] I checked normal vs portable vs custom data directory.
- [ ] I did not attach confidential credentials, GAL exports, logs or mail.
- [ ] I read [繁體中文安裝手冊](../../docs/legacy-exchange-gal-user-manual.zh-TW.md) and [Helpdesk Runbook](../../docs/legacy-exchange-gal-support-runbook.zh-TW.md).
