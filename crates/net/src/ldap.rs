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

/// A maximum of two representations of a name go into ONE LDAP query:
/// preserve actual aliases with spaces, and also try underscores for later
/// mailbox naming. Do not invent punctuation, remove all separators, or
/// infer SMTP addresses: historic aliases do not consistently follow names.
fn search_forms(query: &str) -> Vec<String> {
    let trimmed = query.trim();
    let mut forms = vec![escape_filter_value(trimmed)];
    if trimmed.contains(' ') && !trimmed.contains('_') {
        forms.push(escape_filter_value(&trimmed.replace(' ', "_")));
    } else if trimmed.contains('_') && !trimmed.contains(' ') {
        forms.push(escape_filter_value(&trimmed.replace('_', " ")));
    }
    forms
}

/// Candidate recipient search, not exact GAL ACL/recipient-filter equivalence.
/// One bounded OR-filter; no additional LDAP round trips for name/alias variants.
pub fn recipient_filter(schema: LdapSchema, query: &str) -> Option<String> {
    let trimmed = query.trim();
    if trimmed.is_empty() || trimmed.chars().count() > MAX_QUERY_CHARS {
        return None;
    }
    let variants = search_forms(trimmed);
    let mut terms = Vec::new();
    for q in &variants {
        // Normal name/alias prefix searches: avoid expensive leading wildcards.
        terms.push(format!("(cn={q}*)"));
        terms.push(format!("(displayName={q}*)"));
        // A historical alias may contain spaces; later aliases use underscores.
        terms.push(format!("(mailNickname={q}*)"));
        terms.push(format!("(mail={q}*)"));
        if schema == LdapSchema::Exchange55 {
            terms.push(format!("(rfc822Mailbox={q}*)"));
        }
        if schema == LdapSchema::ActiveDirectory {
            terms.push(format!("(sAMAccountName={q}*)"));
            terms.push(format!("(givenName={q}*)"));
            terms.push(format!("(sn={q}*)"));
            terms.push(format!("(proxyAddresses=SMTP:{q}*)"));
            terms.push(format!("(proxyAddresses=smtp:{q}*)"));
        }
    }
    let matches = format!("(|{})", terms.join(""));
    Some(match schema {
        // This is also used when a legacy-style AD replica was selected in the
        // UI. Exclude directory-only contacts without SMTP mail attributes.
        LdapSchema::Exchange55 => format!("(&(|(mail=*)(rfc822Mailbox=*)){matches})"),
        LdapSchema::ActiveDirectory => format!(
            "(&(|(mail=*)(proxyAddresses=SMTP:*))(!(msExchHideFromAddressLists=TRUE))(|(objectCategory=person)(objectCategory=group)){matches})"
        ),
    })
}

/// Legacy substring search is a fallback only when an indexed prefix query
/// found nobody. It preserves searches by middle names and mail substrings.
fn recipient_substring_filter(schema: LdapSchema, query: &str) -> Option<String> {
    let trimmed = query.trim();
    // A substring fallback has a leading wildcard and may scan broadly.
    // Avoid it for short strings which generate many matches and load the PDC.
    if trimmed.chars().count() < 5 || trimmed.chars().count() > MAX_QUERY_CHARS {
        return None;
    }
    let q = escape_filter_value(trimmed);
    Some(match schema {
        LdapSchema::Exchange55 => {
            format!("(&(|(mail=*)(rfc822Mailbox=*))(|(cn=*{q}*)(displayName=*{q}*)(mailNickname=*{q}*)(mail=*{q}*)(rfc822Mailbox=*{q}*)))")
        }
        LdapSchema::ActiveDirectory => format!(
            "(&(|(mail=*)(proxyAddresses=SMTP:*))(!(msExchHideFromAddressLists=TRUE))(|(objectCategory=person)(objectCategory=group))(|(cn=*{q}*)(displayName=*{q}*)(mail=*{q}*)(mailNickname=*{q}*)(sAMAccountName=*{q}*)(givenName=*{q}*)(sn=*{q}*)(proxyAddresses=*{q}*)))"
        ),
    })
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
    if reply.1.rc == 0
        && reply.0.is_empty()
        && let Some(fallback) = recipient_substring_filter(settings.schema, query)
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

/// A signed/sealed LDAP session is kept in the account worker's thread.
/// Starting Windows PowerShell and negotiating SSPI on *each keystroke* was
/// the dominant avoidable client overhead on legacy Exchange installations.
#[cfg(windows)]
struct WindowsLdapSession {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    replies: std::sync::mpsc::Receiver<std::io::Result<String>>,
}

#[cfg(windows)]
impl Drop for WindowsLdapSession {
    fn drop(&mut self) {
        // Do not leave an invisible long-running PowerShell process behind.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(windows)]
impl WindowsLdapSession {
    fn launch() -> Result<Self> {
        use std::io::BufRead;
        use std::os::windows::process::CommandExt;
        use std::process::{Command, Stdio};
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let mut cmd = Command::new("powershell.exe");
        cmd.args(["-NoProfile", "-NonInteractive", "-Command", include_str!("ldap_windows.ps1")])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW);
        let mut child = cmd.spawn().map_err(|e| NetError::Protocol(format!("cannot launch persistent Windows LDAP worker: {e}")))?;
        let stdin = child.stdin.take().ok_or_else(|| NetError::Protocol("LDAP worker stdin unavailable".into()))?;
        let stdout = child.stdout.take().ok_or_else(|| NetError::Protocol("LDAP worker stdout unavailable".into()))?;
        let (sender, replies) = std::sync::mpsc::sync_channel(2);
        std::thread::spawn(move || {
            for line in std::io::BufReader::new(stdout).lines() {
                let failed = line.is_err();
                if sender.send(line).is_err() || failed {
                    break;
                }
            }
        });
        Ok(Self { child, stdin, replies })
    }

    fn request(&mut self, body: &str) -> Result<String> {
        use std::io::Write;
        self.stdin
            .write_all(body.as_bytes())
            .and_then(|_| self.stdin.write_all(b"\n"))
            .and_then(|_| self.stdin.flush())
            .map_err(|e| NetError::Protocol(format!("cannot send LDAP query to worker: {e}")))?;
        // A timeout kills the worker through Drop. The next query starts a
        // fresh process; no unbounded queue of blocked LDAP requests.
        let reply =
            self.replies.recv_timeout(Duration::from_secs(18)).map_err(|_| NetError::Protocol("Windows LDAP worker response timed out".into()))?;
        reply.map_err(|e| NetError::Protocol(format!("invalid Windows LDAP worker output: {e}")))
    }
}

#[cfg(windows)]
thread_local! {
    static WINDOWS_LDAP_SESSION: std::cell::RefCell<Option<WindowsLdapSession>> = const { std::cell::RefCell::new(None) };
}

/// Windows-native signed/sealed SSPI GAL query. One helper process and bound
/// LDAP connection are reused for successive autocomplete queries on the
/// same account thread. Each request is one UTF-8 JSON line, never shell code.
#[cfg(windows)]
fn windows_integrated_search(settings: &LdapSettings, query: &str) -> Result<Vec<(String, String)>> {
    #[derive(serde::Deserialize)]
    struct Person {
        name: String,
        email: String,
    }
    #[derive(serde::Deserialize)]
    struct Response {
        people: Vec<Person>,
        error: Option<String>,
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
        "schema": if settings.schema == LdapSchema::Exchange55 { "exchange55" } else { "ad" },
        "limit": MAX_RESULTS
    })
    .to_string();

    let data = WINDOWS_LDAP_SESSION.with(|state| {
        let mut guard = state.borrow_mut();
        if guard.is_none() {
            *guard = Some(WindowsLdapSession::launch()?);
        }
        let result = guard.as_mut().ok_or_else(|| NetError::Protocol("Windows LDAP worker not running".into()))?.request(&body);
        if result.is_err() {
            // Broken / hung helper must not poison the next lookup.
            *guard = None;
        }
        result
    })?;
    let response: Response = serde_json::from_str(&data).map_err(|e| NetError::Protocol(format!("invalid LDAP helper JSON: {e}")))?;
    if let Some(category) = response.error {
        if category.is_empty() || category.len() > 64 || !category.bytes().all(|c| c.is_ascii_alphanumeric() || c == b':' || c == b'_') {
            return Err(NetError::Auth("Windows LDAP worker returned an invalid error category".into()));
        }
        return Err(NetError::Auth(format!("Windows Integrated LDAP failed ({category})")));
    }
    let mut results = Vec::new();
    for p in response.people.into_iter().take(MAX_RESULTS) {
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
        assert!(recipient_substring_filter(LdapSchema::ActiveDirectory, "tone").is_none());
        let fallback = recipient_substring_filter(LdapSchema::ActiveDirectory, "toney").unwrap();
        assert!(fallback.contains("(displayName=*toney*)"));
        assert!(fallback.contains("(proxyAddresses=*toney*)"));
    }

    #[test]
    fn legacy_alias_formats_are_covered_by_one_prefix_filter() {
        let space = recipient_filter(LdapSchema::ActiveDirectory, "Alice Smith").unwrap();
        assert!(space.contains("(mailNickname=Alice Smith*)"));
        assert!(space.contains("(mailNickname=Alice_Smith*)"));
        assert!(space.contains("(displayName=Alice Smith*)"));
        assert!(!space.contains("(displayName=*Alice Smith*)"));

        let underscore = recipient_filter(LdapSchema::ActiveDirectory, "sam_lee").unwrap();
        assert!(underscore.contains("(mailNickname=sam_lee*)"));
        assert!(underscore.contains("(mailNickname=sam lee*)"));
        assert!(underscore.contains("(displayName=sam lee*)"));

        let special = recipient_filter(LdapSchema::ActiveDirectory, "JennyLee").unwrap();
        assert!(special.contains("(mailNickname=JennyLee*)"));
        // Do not invent spelling variations, or assume email local parts
        // have any fixed relationship to names/aliases.
        assert!(!special.contains("JennyLeigh"));
    }

    #[test]
    fn exchange55_prefix_search_keeps_mail_filter_and_alias_variants() {
        let filter = recipient_filter(LdapSchema::Exchange55, "alice smith").unwrap();
        assert!(filter.contains("(|(mail=*)(rfc822Mailbox=*))"));
        assert!(filter.contains("(mailNickname=alice smith*)"));
        assert!(filter.contains("(mailNickname=alice_smith*)"));
        assert!(!filter.contains("(cn=*alice smith*)"));
        assert!(recipient_substring_filter(LdapSchema::Exchange55, "alice").unwrap().contains("(cn=*alice*)"));
    }

    #[test]
    fn ldap_filter_escapes_alternate_forms_without_injection() {
        let filter = recipient_filter(LdapSchema::ActiveDirectory, "sam_*)(mail=*)").unwrap();
        assert!(filter.contains(r"sam_\2a\29\28mail=\2a\29"));
        assert!(!filter.contains("(mail=*) ("));
        assert!(recipient_filter(LdapSchema::ActiveDirectory, " ").is_none());
    }

    #[cfg(windows)]
    #[test]
    fn persistent_windows_helper_answers_two_requests_without_restart() {
        let mut helper = WindowsLdapSession::launch().expect("Windows LDAP helper must launch");
        // Invalid local parameters, deliberately no network connection.
        let invalid = r#"{"host":"","port":389,"base_dn":"","filter":"(mail=*)","limit":5,"schema":"ad"}"#;
        let first = helper.request(invalid).expect("first helper response");
        let second = helper.request(invalid).expect("second helper response");
        for result in [first, second] {
            let reply: serde_json::Value = serde_json::from_str(&result).expect("one UTF-8 JSON envelope per request");
            assert_eq!(reply["error"], "HelperRuntime");
            assert_eq!(reply["people"].as_array().map(Vec::len), Some(0));
        }
    }

    #[test]
    fn query_limits_and_unicode() {
        assert!(recipient_filter(LdapSchema::Exchange55, " ").is_none());
        assert!(recipient_filter(LdapSchema::Exchange55, &"x".repeat(129)).is_none());
        assert!(recipient_filter(LdapSchema::Exchange55, "王小明").is_some_and(|s| s.contains("王小明")));
    }
}
