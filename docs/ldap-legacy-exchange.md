# Exchange 5.5 / 2003 LDAP Global Address List

> **Operational documentation (Traditional Chinese):**
> [Installation, configuration, data storage and user manual](./legacy-exchange-gal-user-manual.zh-TW.md) |
> [Technical support, backups, troubleshooting and pilot load-test runbook](./legacy-exchange-gal-support-runbook.zh-TW.md).
>
> Documentation-only update as of 2026-11-10: **search filters, cache
> TTLs, Windows worker, query behavior and security settings are unchanged**.
> This remains a Draft POC; no Exchange 5.5 live compatibility or 100-user
> load-test certification is implied.

This branch adds a **read-only LDAP directory source** to MailCraft's existing
recipient completion / Select Names flow. It does not add EWS, CalDAV, mailbox
personal contacts, Exchange public folders, or MAPI/RPC support.

## Configuration (desktop)

1. Add an IMAP/SMTP account for the legacy Exchange mailbox.
2. Open **File > Info > Account settings > Enterprise directory (LDAP)**.
3. Enable LDAP; select Exchange 5.5 or Exchange 2003 / AD.
4. Enter the server URL and Base DN. Optionally enter Bind DN.
5. Save, then type at least three characters in To/Cc/Bcc or use Select Names.

LDAP uses a **separate LDAP password** in the OS keychain when Bind DN is present.
Anonymous directory search is attempted only when Bind DN is empty.
The password is **never** stored in account settings and is not automatically
reused from IMAP/SMTP. Prefer a dedicated least-privilege directory reader,
not Domain Administrator. For legacy endpoints lacking TLS, use an isolated
TLS-terminating LDAP gateway with controlled access; never transmit privileged
credentials using unencrypted LDAP.

### Example settings

| Server | URL example | Base DN example | Schema |
| --- | --- | --- | --- |
| Exchange 5.5 | ldaps://exchange55.example.com:636 | o=Example | exchange55 |
| Exchange 2003 AD Global Catalog | ldaps://gc.example.com:3269 | DC=example,DC=com | ad |

The examples are **illustrative**, not universal. Exchange 5.5 LDAP port 389
can be changed to 390 in coexistence installations. Its Base DN is normally
not an Active Directory DC= form. Verify actual server, port and search
base from your working Thunderbird configuration or directory administrator.

**Transport security:** LDAPS or STARTTLS and certificate verification
remain the default. A new explicit **legacy cleartext anonymous** option
allows `ldap://` without STARTTLS only when **both Bind DN and password are
absent**. LDAP requests, responses, names, and email addresses are readable on
the network; use it only on an isolated LAN and do not expose the port publicly.
A directory that requires authentication over unencrypted LDAP **cannot be
used directly** in this mode. Use a separate tightly controlled legacy bridge
and a dedicated low-privilege read-only account, never Domain Administrator.
This is not a silent fallback from failed TLS.

**Exchange 5.5 compatibility caveat:** The chosen LDAP library speaks LDAPv3
and can search anonymously over cleartext LDAP only after explicit opt-in.
Some Exchange 5.5 sites use LDAPv2 or require credentialed binds without TLS;
those sites need an isolated, secured compatibility service run by administrators.

### CLI alternative

The session command can be invoked using the regular MailCraft CLI:

```bash
mailcraft-cli run account.list '{}'
mailcraft-cli run account.ldap '{"account":"a1","url":"ldaps://gc.example.com:3269","baseDn":"DC=example,DC=com","bindDn":"user@example.com","schema":"ad"}'
mailcraft-cli run contact.searchDirectory '{"query":"alice"}'
```

To switch off LDAP:

```bash
mailcraft-cli run account.ldap '{"account":"a1","disable":true}'
```

Use the same data directory as your desktop account. The app and the CLI
should not write to the same data store concurrently.

## Schema and GAL correctness

- Exchange 5.5 uses the Exchange Directory Service; the adapter prefers
  `rfc822Mailbox` with fallback to `mail`.
- Exchange 2003 queries AD/GC and uses `mail` and the
  `msExchHideFromAddressLists` flag.
- This is a **candidate recipient filter**, not a full implementation of
  an organization's custom GAL query, Address Book View rules or ACLs.
- Recipient groups are returned by their SMTP address; expanding their
  membership is not implemented.
- Entries are deduplicated by SMTP address and capped at 50 results.
- A 300-second positive and 30-second negative cache are already provided
  by MailCraft's directory search layer.

## Development / testing

From the repository root, with Rust 1.98 and network access:

```bash
cargo generate-lockfile
cargo fmt --all
cargo test -p mailcraft-model
cargo test -p mailcraft-net ldap
cargo test -p mailcraft-engine directory
cargo xtask ci
```

The branch preparation workflow regenerates `Cargo.lock` and applies rustfmt;
verify the changes are committed before using `--locked` in a production build.

Do not use a production Exchange 5.5/2003 server as the initial test target.
Start with a disposable LDAPv3/LDAPS directory containing synthetic people;
then use an isolated read-only organizational test account.

### Acceptance checklist

- Settings survive a restart without storing credentials in JSON.
- The certificate chain and server name are checked.
- Authenticated LDAP bind works only after STARTTLS or LDAPS is established; plaintext LDAP is anonymous-only.
- Searches in To, Cc, Bcc and Select Names return the same contacts.
- Filter injection, invalid SMTP values, duplicate entries and 50-result
  truncation are handled.
- Exchange 2003 hidden recipients are excluded; custom GAL membership and
  Exchange 5.5 attributes are verified with the site's administrator.
- Existing EWS/Graph GAL searches and IMAP/SMTP operations remain unchanged.

## Known limitations

This branch has not been compiled or exercised against real servers in the
authoring environment. A complete release requires dependency lockfile
regeneration, CI, and LDAP protocol interoperability testing.

## Site configuration caution

The directory controller or PDC endpoint is a **site-specific configuration**, not a
shippable default. The host of the Exchange 2003 mailbox may differ from the
LDAP directory host. This private address must not be hard-coded in source;
use administrator-provided settings. A classic Windows NT PDC is not
necessarily an AD LDAP service: verify the directory type, Root DSE, and
STARTTLS support before choosing the AD schema.


## Cleartext legacy configuration (anonymous only)

```json
{
  "account": "a1",
  "url": "ldap://directory.example.com:389",
  "baseDn": "DC=example,DC=com",
  "schema": "ad",
  "bindDn": null,
  "allowInsecureAnonymous": true
}
```

The domain-controller endpoint from Thunderbird is separate from the Exchange
mail host. If anonymous search returns LDAP result 50 (insufficient access),
**do not** enter the administrator password into an unencrypted LDAP profile.
Use a dedicated gateway and a least-privilege service account instead.

## Windows Integrated LDAP — recommended for domain-joined desktops

The site verified that anonymous search is rejected but a normal employee
Windows session succeeds using `Negotiate`, LDAP Signing and LDAP Sealing.
This is preferred over an unencrypted administrator Simple Bind.

For an Exchange 2003 Active Directory GAL, use a **directory controller**
rather than the Exchange mail host. Example (replace placeholders):

```json
{
  "account": "a1",
  "url": "ldap://directory.example.com:389",
  "baseDn": "DC=example,DC=com",
  "schema": "ad",
  "windowsIntegrated": true,
  "allowInsecureAnonymous": false,
  "bindDn": null
}
```

Desktop settings: select **Windows Integrated (current domain user)**.
No LDAP password is entered or stored. The native Windows LDAP layer uses the
logged-in domain identity and requests Negotiate with signing and sealing.
MailCraft uses a bundled, fixed PowerShell helper with all inputs sent in JSON
over stdin; user input is never interpreted as PowerShell source.

The standalone PowerShell probe `scripts/Test-LegacyGal.ps1` has been tested
successfully on a domain-joined client; the new MailCraft Windows connector
still needs a **real end-to-end smoke test** on that client. Linux/macOS
clients cannot use the Windows Integrated mode.

Do not assume that a successful Exchange 2003 AD lookup verifies Exchange
5.5 Directory Service, which may use different LDAP versions and recipient
attributes. The full GAL membership and distribution-list behavior still
require local acceptance testing.

## Recipient autocomplete: source badges and GAL-only mode

The desktop compose To/Cc/Bcc autocomplete now identifies each suggestion
as **GAL**, **Contacts** or **Recent**:

- **GAL**: the address is present in the latest directory search result, even
  when it also occurs in message history.
- **Contacts**: saved in a personal address book, and not currently returned
  by the directory query.
- **Recent**: learned from mail sent or received, and not matched in either
  preceding source.

The **GAL only** checkbox beneath recipient boxes is **off by default**.
When enabled, suggestions come exclusively from `address_book("directory")`
and do not include stored Contacts or message history. The user can still
manually enter any valid address, so it is *not* an outbound-domain policy.
The toggle belongs to the compose message; it does not rewrite account
settings or delete existing contacts.

An external email domain does not establish that an address is absent from
the organisation GAL: Active Directory can contain mail contacts and other
permitted address domains. Only the actual directory search is authoritative
for this source badge. No hardcoded domain allowlist is used.

The first directory query starts once at least three characters are typed.
While a query is still in progress, suggestions may temporarily be empty in
GAL-only mode. The existing short-term directory cache still applies.

## Troubleshooting GAL-only omissions and source labels

- With **GAL only**, enter at least **3 characters**. The compose form now
  distinguishes "Searching GAL…" from "No GAL results" and directory failures.
  Wait for a search to finish before assuming a name is absent.
- The AD LDAP filter now searches `displayName`, `cn`, `givenName`, `sn`,
  `mail`, `mailNickname`, `sAMAccountName`, and `proxyAddresses`.
  The adapter can return a primary SMTP proxy when `mail` is empty.
- A person present in a custom Outlook GAL can still be missed if the Base DN
  points at a different domain, if directory permissions differ, if hidden
  address-list rules differ, or if the directory server limits results. Test
  full names, account aliases and complete SMTP addresses before changing AD.
- Source label **Mail** means an address appears in the synchronized message
  index, which includes **incoming senders** as well as recipients. It does
  not claim the user recently contacted the address. **Checking** means its
  GAL membership has not been resolved yet by the directory query.
- **Contacts** comes from saved contacts, and **GAL** is based on an actual
  matching directory reply; neither is inferred from an email domain.

## Performance architecture for slow legacy PDCs

The Windows integrated connector now uses **one hidden PowerShell worker process
per account-worker thread**, with a **persistent SSPI-authenticated LDAP
connection**, rather than launching a full PowerShell process and negotiating
Windows authentication for every completed recipient search. That persistent
connection requests both Signing and Sealing. The process is terminated when
its owning worker thread exits or if a request exceeds the 18-second local
deadline; a failed LDAP connection is disposed and recreated for a subsequent
query.

AD searches are **index-friendly prefix searches first**: display name,
first/last name, alias, logon name, email and SMTP proxy prefix. The old
leading-wildcard substring search happens **only if no result was found** and
the query has **at least five characters**; its result count stays capped at
50 and server time at eight seconds per request. Names found by the ordinary
prefix filter normally cost one LDAP search, while a miss with five or more
characters may still cost two. Searching a name fragment *in the middle* of
a name with fewer than five characters will not match this optimized path.

In the UI, query input is debounced for 350 ms and only the settled input
submits an LDAP job. The existing in-memory directory cache retains successful
answers for 300 seconds and empty/error answers for 30 seconds, and it keeps
at most 200 query entries. No full GAL sync, periodic polling or broad
unbounded downloads are performed.

### Acceptance / benchmarking

Test on a normal domain-joined Windows account with GAL-only enabled:
1. Start MailCraft and type a known exact prefix (e.g. first name); measure
   the first result, including initial worker startup.
2. Repeat the same prefix immediately: the cached answer should appear
   without another LDAP request.
3. Search another employee: the SSPI helper and connection should be reused,
   so it avoids the previous repeated PowerShell startup cost.
4. Check employee aliases and a five-character middle-name fragment (fallback)
   to verify matching fidelity, and retry following a long idle period.
5. Watch Task Manager: expect a single hidden PowerShell worker for an active
   Windows LDAP account worker (not one new process per lookup). Compare
   process count and response latency with the previous build.

The approach specifically avoids any GAL-wide scrape or server-side schema,
ACL, index, or registry change on the legacy PDC.


### 2026-11-11: conservative alias/name prefix matching (pilot)

One request can search a name in its **literal form** and one additional
separator variant. For example, `Alice Smith` probes `Alice Smith*` and
`Alice_Smith*` in the same bounded OR filter, checking real directory
`mailNickname`, `displayName`, `cn`, `mail`, and version-appropriate
address attributes. Conversely, `Alice_Smith` can match a historic Alias
containing a space. This is **not** a general punctuation/romanization
normalizer: old aliases can be independent abbreviations or misspellings;
always use the server-provided SMTP address. Names with no separator remain
unchanged, and the client must not invent mail addresses.

Prefer prefix queries to avoid leading-wildcard LDAP scans. A **single**
substring fallback is still available only when the prefix lookup succeeds
with *zero* returned directory records and the string has **five or more
characters**. No additional fallback on timeouts/errors/full result sets.
This rule now applies to both directory schemas, with the appropriate
schema-specific attribute filter. A missing usable SMTP field is not
automatically repaired by guessing a domain.

The returned entries are capped at 50, server search time at 8 seconds
per search; Windows integrated mode reuses the existing signed/sealed
PowerShell/SSPI helper, with a bounded local wait. Input debounce and cache
TTLs **are unchanged**. No schema, index, account, or directory
configuration is modified server-side.

The source includes synthetic tests for aliases containing spaces,
underscores, aliases without separators, and LDAP filter escaping. **Do not
include employee names, email addresses, internal directory IPs, raw
directory dumps or credentials in public test fixtures**. This change
remains a Draft POC pending CI, signed/sealed Windows smoke testing and
field validation before broad rollout.
