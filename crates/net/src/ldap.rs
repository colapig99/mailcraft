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
        LdapSchema::ActiveDirectory => format!(
            "(&(mail=*)(!(msExchHideFromAddressLists=TRUE))(|(objectCategory=person)(objectCategory=group))(|(cn=*{q}*)(displayName=*{q}*)(mail=*{q}*)))"
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
    let mut conn = LdapConn::with_settings(opts, url).map_err(|e| NetError::Tls(format!("secure LDAP connection failed: {e}")))?;
    conn.with_timeout(Duration::from_secs(12));
    Ok(conn)
}

/// Read-only GAL search. An optional bind DN uses the mailbox password held in
/// the OS keychain; separate LDAP service credentials are not supported yet.
/// LDAPv2-only Exchange 5.5 installations require an isolated compatibility bridge.
pub fn search_directory(settings: &LdapSettings, password: Option<&str>, query: &str) -> Result<Vec<(String, String)>> {
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
        LdapSchema::ActiveDirectory => vec!["cn", "displayName", "mail"],
    };
    let reply = conn
        .with_search_options(SearchOptions::new().sizelimit(MAX_RESULTS as i32).timelimit(8))
        .search(&settings.base_dn, Scope::Subtree, &filter, attrs)
        .map_err(|e| NetError::Protocol(format!("LDAP search failed: {e}")))?;
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
        let Some(email) = first_attr(&entry, wanted) else { continue };
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
    fn query_limits_and_unicode() {
        assert!(recipient_filter(LdapSchema::Exchange55, " ").is_none());
        assert!(recipient_filter(LdapSchema::Exchange55, &"x".repeat(129)).is_none());
        assert!(recipient_filter(LdapSchema::Exchange55, "王小明").is_some_and(|s| s.contains("王小明")));
    }
}
