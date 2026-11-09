# Exchange 5.5 / 2003 LDAP Global Address List

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

**Transport security**: the client only permits `ldaps://` or `ldap://`
with mandatory STARTTLS and certificate verification. Import the enterprise
CA into the operating system trust store if needed. Never disable verification
or expose an obsolete Exchange server to the public internet.

**Exchange 5.5 compatibility caveat:** The chosen LDAP library speaks LDAPv3
and requires modern TLS. Some Exchange 5.5 sites cannot negotiate it, or only
support LDAPv2. Such sites need an isolated, secured compatibility service
run by administrators; this code intentionally does not downgrade.

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
- LDAP bind works only after STARTTLS or LDAPS is established.
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

A reported domain controller/PDC at 10.10.2.31:389 is a **site fact**, not a
shippable default. The host of the Exchange 2003 mailbox may differ from the
LDAP directory host. This private address must not be hard-coded in source;
use administrator-provided settings. A classic Windows NT PDC is not
necessarily an AD LDAP service: verify the directory type, Root DSE, and
STARTTLS support before choosing the AD schema.
