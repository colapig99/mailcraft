//! LDAPv3 GAL lookup for legacy Exchange. Verified TLS is the default.
//! A deliberately enabled cleartext mode is anonymous-only: no credentials,
//! no LDAP simple bind, and no TLS downgrade on connection errors.

use crate::{NetError, Result};
use ldap3::{LdapConn, LdapConnSettings, Scope, SearchEntry, SearchOptions};
use mailcraft_model::{LdapSchema, LdapSettings};
use std::time::Duration;

const MAX_RESULTS: usize = 50;
pub const MAX_QUERY_CHARS: usize = 128;

/// RFC 4515 escapes for user-controlled LDAP filter fragments.
pub fn escape_filter_value(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '*' => escaped.push_str(r"\2a"),
            '(' => escaped.push_str(r"\28"),
            ')' => escaped.push_str(r"\29"),
            '\\' => escaped.push_str(r"\5c"),
            '\0' => escaped.push_str(r"\00"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

/// Candidate recipient search, not exact GAL ACL/recipient-filter equivalence.
pub fn recipient_filter(schema: LdapSchema, query: &str) -> Option<String> {
    let trimmed = query.trim();
    if trimmed.is_empty() || trimmed.chars().count() > MAX_QUERY_CHARS {
        return None;
    }
    let q = escape_filter_value(trimmed);
    Some(match schema {
        LdapSchema::Exchange55 => format!("(&(|(mail=*)(rfc822Mailbox=*))(|(cn=*{q}*)(displayName=*{q}*)(mail=*{q}*)(rfc822Mailbox=*{q}*)))"),
        // Prefix filters can use AD attribute indexes. Leading wildcards
        // require broad scans and are expensive on legacy domain controllers.
        // On a miss, search_directory uses the older substring filter as a
        // compatibility fallback (e.g. searching the middle of a display name).
        LdapSchema::ActiveDirectory => format!(
            "(&(|(mail=*)(proxyAddresses=SMTP:*))(!(msExchHideFromAddressLists=TRUE))(|(objectCategory=person)(objectCategory=group))(|(cn={q}*)(displayName={q}*)(mail={q}*)(mailNickname={q}*)(sAMAccountName={q}*)(givenName={q}*)(sn={q}*)(proxyAddresses=SMTP:{q}*)(proxyAddresses=smtp:{q}*)))"
        ),
    })
}

/// Legacy substring search is a fallback only when an indexed prefix query
/// found nobody. It preserves searches by middle names and mail substrings.
fn recipient_substring_filter(query: &str) -> Option<String> {
    let trimmed = query.trim();
    if trimmed.is_empty() || trimmed.chars().count() > MAX_QUERY_CHARS {
        return None;
    }
    let q = escape_filter_value(trimmed);
    Some(format!(
        "(&(|(mail=*)(proxyAddresses=SMTP:*))(!(msExchHideFromAddressLists=TRUE))(|(objectCategory=person)(objectCategory=group))(|(cn=*{q}*)(displayName=*{q}*)(mail=*{q}*)(mailNickname=*{q}*)(sAMAccountName=*{q}*)(givenName=*{q}*)(sn=*{q}*)(proxyAddresses=*{q}*)))"
    ))
}

/// Require LDAPS or STARTTLS by default. If explicitly configured, permit
/// cleartext LDAP only for anonymous searching; never downgrade on TLS errors.
fn ldap_connection(settings: &LdapSettings) -> Result<LdapConn> {
    if settings.allow_insecure_anonymous && settings.bind_dn.as_deref().is_some_and(|s| !s.trim().is_empty()) {
        return Err(NetError::Refused("unencrypted LDAP cannot use a Bind DN or password".into()));
    }
    let url = settings.url.trim();
    let starttls = if url.starts_with("ldaps://") {
        false
    } else if url.starts_with("ldap://") {
        !settings.allow_insecure_anonymous
    } else {
        return Err(NetError::Refused("LDAP URL must use ldaps:// or ldap://".into()));
    };
    if settings.allow_insecure_anonymous && !url.starts_with("ldap://") {
        return Err(NetError::Refused("anonymous cleartext mode requires ldap://".into()));
    }
    // Endpoint only: refuse credentials embedded in URLs and LDAP URL query/filter parts.
    let authority = url.strip_prefix("ldaps://").or_else(|| url.strip_prefix("ldap://")).unwrap_or("");
    if authority.is_empty() || authority.chars().any(|ch| matches!(ch, '/' | '?' | '#' | '@')) || authority.chars().any(char::is_whitespace) {
        return Err(NetError::Refused("LDAP URL must contain only a server and optional port".into()));
    }
    if settings.base_dn.trim().is_empty() {
        return Err(NetError::Refused("LDAP Base DN is required".into()));
    }
    let opts = LdapConnSettings::new().set_conn_timeout(Duration::from_secs(10)).set_starttls(starttls);
    let mut conn = LdapConn::with_settings(opts, url).map_err(|e| NetError::Protocol(format!("LDAP connection failed: {e}")))?;
    conn.with_timeout(Duration::from_secs(12));
    Ok(conn)
}

/// Read-only GAL search. A Bind DN uses a separate system-keychain secret.
/// Windows integrated mode delegates authentication to the OS and requests
/// signing and sealing rather than putting a password on the LDAP wire.
pub fn search_directory(settings: &LdapSettings, password: Option<&str>, query: &str) -> Result<Vec<(String, String)>> {
    if settings.windows_integrated {
        if password.is_some() || settings.bind_dn.is_some() || settings.allow_insecure_anonymous || !settings.url.starts_with("ldap://") {
            return Err(NetError::Refused("Windows Integrated LDAP needs ldap://, no Bind DN and no password".into()));
        }
        #[cfg(windows)]
        {
            return windows_integrated_search(settings, query);
        }
        #[cfg(not(windows))]
        {
            return Err(NetError::Refused("Windows Integrated LDAP is available only on Windows".into()));
        }
    }
    if settings.allow_insecure_anonymous && (settings.bind_dn.as_deref().is_some_and(|s| !s.trim().is_empty()) || password.is_some()) {
        return Err(NetError::Refused("cleartext LDAP supports anonymous queries only; passwords are forbidden".into()));
    }
    let filter = recipient_filter(settings.schema, query).ok_or_else(|| NetError::Refused("LDAP search must contain 1–128 characters".into()))?;
    let mut conn = ldap_connection(settings)?;
    if let Some(bind_dn) = settings.bind_dn.as_deref().filter(|s| !s.trim().is_empty()) {
        let secret = password
            .filter(|s| !s.is_empty())
            .ok_or_else(|| NetError::Auth("LDAP Bind DN needs mailbox credentials in the system keychain".into()))?;
        conn.simple_bind(bind_dn, secret)
            .map_err(|e| NetError::Auth(format!("LDAP bind failed: {e}")))?
            .success()
            .map_err(|e| NetError::Auth(format!("LDAP bind rejected: {e}")))?;
    }
    let attrs = match settings.schema {
        LdapSchema::Exchange55 => vec!["cn", "displayName", "mail", "rfc822Mailbox"],
        LdapSchema::ActiveDirectory => vec!["cn", "displayName", "mail", "proxyAddresses"],
    };
    let mut reply = conn
        .with_search_options(SearchOptions::new().sizelimit(MAX_RESULTS as i32).timelimit(8))
        .search(&settings.base_dn, Scope::Subtree, &filter, attrs.clone())
        .map_err(|e| NetError::Protocol(format!("LDAP search failed: {e}")))?;
    // The fast indexed search handles ordinary name/alias prefixes. One
    // substring retry is allowed only for a successful empty AD response;
    // do not retry on an LDAP error, timeout or server-side result cap.
    if settings.schema == LdapSchema::ActiveDirectory && reply.1.rc == 0 && reply.0.is_empty()
        && let Some(fallback) = recipient_substring_filter(query)
    {
        reply = conn
            .with_search_options(SearchOptions::new().sizelimit(MAX_RESULTS as i32).timelimit(8))
            .search(&settings.base_dn, Scope::Subtree, &fallback, attrs)
            .map_err(|e| NetError::Protocol(format!("LDAP fallback search failed: {e}")))?;
    }
    // LDAP result 4 (sizeLimitExceeded) can still contain the first 50 valid
    // entries; show that bounded prefix instead of incorrectly saying no one exists.
    if reply.1.rc != 0 && !(reply.1.rc == 4 && !reply.0.is_empty()) {
        return Err(NetError::Protocol(format!("LDAP search rejected: {}", reply.1)));
    }
    let entries = reply.0;
    let mut people = Vec::new();
    for raw in entries.into_iter().take(MAX_RESULTS) {
        let entry = SearchEntry::construct(raw);
        let wanted: &[&str] = match settings.schema {
            LdapSchema::Exchange55 => &["rfc822Mailbox", "mail"],
            LdapSchema::ActiveDirectory => &["mail"],
        };
        let Some(email) =
            first_attr(&entry, wanted).or_else(|| if settings.schema == LdapSchema::ActiveDirectory { primary_proxy_address(&entry) } else { None })
        else {
            continue;
        };
        if !mailcraft_model::addr::is_valid_email(&email) {
            continue;
        }
        let name = first_attr(&entry, &["displayName", "cn"]).unwrap_or_default();
        if !people.iter().any(|(_, known): &(String, String)| known.eq_ignore_ascii_case(&email)) {
            people.push((name, email));
        }
    }
    let _ = conn.unbind();
    Ok(people)
}

fn first_attr(entry: &SearchEntry, names: &[&str]) -> Option<String> {
    for wanted in names {
        if let Some((_, values)) = entry.attrs.iter().find(|(key, _)| key.eq_ignore_ascii_case(wanted))
            && let Some(s) = values.iter().find(|s| !s.trim().is_empty())
        {
            return Some(s.trim().to_owned());
        }
    }
    None
}

/// The primary SMTP proxy uses uppercase SMTP:; lowercase smtp: aliases
/// are fallback candidates when an AD mail attribute is absent.
fn primary_proxy_address(entry: &SearchEntry) -> Option<String> {
    let (_, values) = entry.attrs.iter().find(|(key, _)| key.eq_ignore_ascii_case("proxyAddresses"))?;
    values
        .iter()
        .find_map(|v| v.strip_prefix("SMTP:").map(str::to_string))
        .or_else(|| values.iter().find_map(|v| v.strip_prefix("smtp:").map(str::to_string)))
}

/// Search through Windows' built-in LDAP stack with SSPI Negotiate and
/// directory signing/sealing. The helper script is a fixed build-time resource;
/// user input travels solely as JSON through stdin (never shell interpolation).
#[cfg(windows)]
fn windows_integrated_search(settings: &LdapSettings, query: &str) -> Result<Vec<(String, String)>> {
    use std::io::{Read, Write};
    use std::process::{Command, Stdio};

    #[derive(serde::Deserialize)]
    struct Person {
        name: String,
        email: String,
    }

    let filter = recipient_filter(settings.schema, query).ok_or_else(|| NetError::Refused("LDAP query must contain 1–128 characters".into()))?;
    let host_port = settings.url.strip_prefix("ldap://").ok_or_else(|| NetError::Refused("Windows Integrated LDAP requires ldap://".into()))?;
    if host_port.is_empty() || host_port.chars().any(|c| c.is_whitespace() || matches!(c, '/' | '?' | '#' | '@')) {
        return Err(NetError::Refused("LDAP URL must contain only a host and optional port".into()));
    }
    let (host, port) = match host_port.rsplit_once(':') {
        Some((host, digits)) if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) => {
            (host, digits.parse::<u16>().map_err(|_| NetError::Refused("invalid LDAP port".into()))?)
        }
        Some(_) => return Err(NetError::Refused("Windows Integrated LDAP requires a host or host:port".into())),
        None => (host_port, 389),
    };
    if host.is_empty() || settings.base_dn.trim().is_empty() {
        return Err(NetError::Refused("LDAP host and Base DN are required".into()));
    }
    let body = serde_json::json!({
        "host": host, "port": port, "base_dn": settings.base_dn,
        "filter": filter, "fallback_filter": if settings.schema == LdapSchema::ActiveDirectory { recipient_substring_filter(query) } else { None },
        "schema": if settings.schema == LdapSchema::Exchange55 { "exchange55" } else { "ad" }, "limit": MAX_RESULTS
    })
    .to_string();

    use std::os::windows::process::CommandExt;

    // Windows creates a console window for a console child of a GUI app unless
    // explicitly told not to. That was causing a visible PowerShell popup on
    // every GAL autocomplete request.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut cmd = Command::new("powershell.exe");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", include_str!("ldap_windows.ps1")])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd.creation_flags(CREATE_NO_WINDOW);
    let mut child = cmd.spawn().map_err(|e| NetError::Protocol(format!("cannot launch Windows LDAP helper: {e}")))?;
    let Some(mut input) = child.stdin.take() else {
        return Err(NetError::Protocol("cannot open LDAP helper input".into()));
    };
    input.write_all(body.as_bytes()).map_err(|e| NetError::Protocol(format!("cannot submit LDAP search: {e}")))?;
    drop(input);
    // Read concurrently while waiting: a full stdout pipe must never block the
    // helper and leave an invisible PowerShell process running indefinitely.
    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err(NetError::Protocol("cannot read Windows LDAP helper output".into()));
    };
    let reader = std::thread::spawn(move || {
        let mut output = Vec::new();
        let read = stdout.take(1_048_576).read_to_end(&mut output);
        (read, output)
    });
    let stderr_reader = child.stderr.take().map(|stderr| {
        std::thread::spawn(move || {
            let mut details = Vec::new();
            let _ = stderr.take(4096).read_to_end(&mut details);
            details
        })
    });
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if std::time::Instant::now() < deadline => std::thread::sleep(Duration::from_millis(50)),
            _ => break None,
        }
    };
    if status.is_none() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let output = reader.join().map_err(|_| NetError::Protocol("Windows LDAP output reader failed".into()))?;
    let diagnostics = stderr_reader.and_then(|thread| thread.join().ok()).unwrap_or_default();
    if status.is_none() {
        return Err(NetError::Protocol("Windows LDAP query timed out after 20 seconds".into()));
    }
    output.0.map_err(|e| NetError::Protocol(format!("cannot read LDAP results: {e}")))?;
    if !status.is_some_and(|s| s.success()) {
        // Only accept a category emitted by our fixed helper; never echo
        // arbitrary PowerShell stderr (potentially includes directory data).
        let reason = std::str::from_utf8(&diagnostics)
            .ok()
            .and_then(|err| err.lines().find_map(|line| line.trim().strip_prefix("LDAP_HELPER_ERROR:")))
            .filter(|category| {
                !category.is_empty() && category.len() <= 64 && category.bytes().all(|c| c.is_ascii_alphanumeric() || c == b':' || c == b'_')
            })
            .unwrap_or("Unknown");
        return Err(NetError::Auth(format!(
            "Windows Integrated LDAP helper failed ({reason}); check the directory settings and Windows domain login"
        )));
    }
    let people: Vec<Person> = serde_json::from_slice(&output.1).map_err(|e| NetError::Protocol(format!("invalid LDAP helper response: {e}")))?;
    let mut results = Vec::new();
    for p in people.into_iter().take(MAX_RESULTS) {
        if !mailcraft_model::addr::is_valid_email(&p.email) {
            continue;
        }
        if !results.iter().any(|(_, email): &(String, String)| email.eq_ignore_ascii_case(&p.email)) {
            results.push((p.name, p.email));
        }
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filter_metacharacters_are_escaped() {
        assert_eq!(escape_filter_value("a*(b)\\\0"), r"a\2a\28b\29\5c\00");
    }
    #[test]
    fn legacy_and_ad_filters_are_distinct() {
        let old = recipient_filter(LdapSchema::Exchange55, "Alice");
        let ad = recipient_filter(LdapSchema::ActiveDirectory, "Alice");
        assert!(old.as_deref().is_some_and(|s| s.contains("rfc822Mailbox")));
        assert!(ad.as_deref().is_some_and(|s| s.contains("msExchHideFromAddressLists")));
    }
    #[test]
    fn injection_is_escaped() {
        let f = recipient_filter(LdapSchema::ActiveDirectory, "a*)(mail=*)");
        assert!(f.as_deref().is_some_and(|s| s.contains(r"a\2a\29\28mail=\2a\29")));
    }
    #[test]
    fn plaintext_directory_mode_is_explicit_and_anonymous_only() {
        let settings = LdapSettings {
            url: "ldap://192.0.2.1:389".into(),
            base_dn: "dc=example,dc=com".into(),
            allow_insecure_anonymous: true,
            ..LdapSettings::default()
        };
        // Must reject before any network I/O: no password on a cleartext wire.
        assert!(search_directory(&settings, Some("secret"), "Alice").is_err());
        let secured_bind = LdapSettings { bind_dn: Some("cn=reader,dc=example,dc=com".into()), ..settings };
        assert!(search_directory(&secured_bind, None, "Alice").is_err());
    }
    #[test]
    fn ad_directory_search_includes_alias_and_logon_name() {
        let filter = recipient_filter(LdapSchema::ActiveDirectory, "toney").unwrap();
        assert!(filter.contains("(sAMAccountName=toney*)"));
        assert!(filter.contains("(mailNickname=toney*)"));
        assert!(filter.contains("(proxyAddresses=SMTP:toney*)"));
        assert!(filter.contains("(givenName=toney*)"));
        assert!(filter.contains("(sn=toney*)"));
        assert!(!filter.contains("(displayName=*toney*)"));
        let fallback = recipient_substring_filter("toney").unwrap();
        assert!(fallback.contains("(displayName=*toney*)"));
        assert!(fallback.contains("(proxyAddresses=*toney*)"));
    }

    #[test]
    fn query_limits_and_unicode() {
        assert!(recipient_filter(LdapSchema::Exchange55, " ").is_none());
        assert!(recipient_filter(LdapSchema::Exchange55, &"x".repeat(129)).is_none());
        assert!(recipient_filter(LdapSchema::Exchange55, "王小明").is_some_and(|s| s.contains("王小明")));
    }
}
