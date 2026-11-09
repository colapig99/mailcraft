//! LDAP directory query helpers for legacy Exchange GAL integration.
//!
//! This module deliberately performs no network I/O. The eventual transport must use
//! authenticated, protected LDAP connections and bound server-side result limits.
//! Never interpolate an unescaped user query into an LDAP filter.

/// Directory schema whose SMTP address attributes differ between Exchange releases.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirectorySchema {
    /// Exchange 5.5's own Directory Service (not Active Directory).
    Exchange55,
    /// Exchange 2003 recipient objects stored in Active Directory.
    ActiveDirectory,
}

/// Maximum characters accepted from a recipient search field.
pub const MAX_QUERY_CHARS: usize = 128;

/// RFC 4515 filter-value escaping, preserving UTF-8 characters.
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

/// A conservative recipient search, not an exact replacement for a server's
/// custom GAL recipient filter. Site-specific GAL restrictions must be applied
/// by the directory provider before the results are displayed.
///
/// Returns None for empty or excessively long searches.
pub fn recipient_filter(schema: DirectorySchema, query: &str) -> Option<String> {
    let trimmed = query.trim();
    if trimmed.is_empty() || trimmed.chars().count() > MAX_QUERY_CHARS {
        return None;
    }
    let q = escape_filter_value(trimmed);
    Some(match schema {
        DirectorySchema::Exchange55 => format!(
            "(&(|(mail=*)(rfc822Mailbox=*))(|(cn=*{q}*)(displayName=*{q}*)(mail=*{q}*)(rfc822Mailbox=*{q}*)))"
        ),
        DirectorySchema::ActiveDirectory => format!(
            "(&(mail=*)(!(msExchHideFromAddressLists=TRUE))(|(objectCategory=person)(objectCategory=group))(|(cn=*{q}*)(displayName=*{q}*)(mail=*{q}*)))"
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_ldap_filter_metacharacters() {
        assert_eq!(escape_filter_value("a*(b)\\\0"), r"a\2a\28b\29\5c\00");
    }

    #[test]
    fn filters_are_schema_specific() {
        let old = recipient_filter(DirectorySchema::Exchange55, "Alice");
        let ad = recipient_filter(DirectorySchema::ActiveDirectory, "Alice");
        assert!(old.as_deref().is_some_and(|f| f.contains("rfc822Mailbox")));
        assert!(ad.as_deref().is_some_and(|f| f.contains("msExchHideFromAddressLists")));
        assert!(ad.as_deref().is_some_and(|f| !f.contains("rfc822Mailbox")));
    }

    #[test]
    fn query_is_escaped_before_interpolation() {
        let f = recipient_filter(DirectorySchema::ActiveDirectory, "a*)(mail=*)");
        assert!(f.as_deref().is_some_and(|s| s.contains(r"a\2a\29\28mail=\2a\29")));
    }

    #[test]
    fn rejects_empty_and_unbounded_queries() {
        assert_eq!(recipient_filter(DirectorySchema::Exchange55, "  "), None);
        assert_eq!(recipient_filter(DirectorySchema::Exchange55, &"x".repeat(129)), None);
        assert!(recipient_filter(DirectorySchema::Exchange55, &"x".repeat(128)).is_some());
    }

    #[test]
    fn utf8_query_is_retained() {
        let filter = recipient_filter(DirectorySchema::Exchange55, "王小明");
        assert!(filter.as_deref().is_some_and(|s| s.contains("王小明")));
    }
}
