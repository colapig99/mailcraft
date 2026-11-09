//! Read-only enterprise LDAP directory settings (Exchange 5.5 / Exchange 2003).

use super::*;
use mailcraft_engine::model::LdapSchema;

#[derive(Clone, Debug, Default)]
pub struct LdapForm {
    pub account: String,
    pub enabled: bool,
    pub url: String,
    pub base_dn: String,
    pub bind_dn: String,
    pub password: String,
    pub saved_bind_dn: String,
    pub schema: LdapSchema,
    pub allow_insecure_anonymous: bool,
    pub windows_integrated: bool,
    pub error: Option<String>,
}

impl LdapForm {
    pub fn of(c: &AccountConfig) -> Self {
        let ldap = c.ldap.clone().unwrap_or_default();
        Self {
            account: c.id.clone(),
            enabled: c.ldap.is_some(),
            url: ldap.url,
            base_dn: ldap.base_dn,
            saved_bind_dn: ldap.bind_dn.clone().unwrap_or_default(),
            bind_dn: ldap.bind_dn.unwrap_or_default(),
            password: String::new(),
            schema: ldap.schema,
            allow_insecure_anonymous: ldap.allow_insecure_anonymous,
            windows_integrated: ldap.windows_integrated,
            error: None,
        }
    }
}

pub(super) fn draw(ui: &mut Ui, f: &mut LdapForm, t: &Tokens, acts: &mut Vec<Action>) -> bool {
    ui.set_width(490.0);
    title(ui, t, "LDAP enterprise address book");
    ui.label(RichText::new(
        "Search your Exchange 5.5 Directory or Exchange 2003 Active Directory from recipient fields. LDAP is read-only. Default TLS, or Windows domain sign-in with LDAP sealing."
    ).color(t.dim).small());
    ui.add_space(8.0);
    ui.checkbox(&mut f.enabled, "Use LDAP organisation directory");
    if f.enabled {
        ui.horizontal(|ui| {
            ui.label("Directory schema");
            ui.radio_value(&mut f.schema, LdapSchema::Exchange55, "Exchange 5.5");
            ui.radio_value(&mut f.schema, LdapSchema::ActiveDirectory, "Exchange 2003 / AD");
        });
        let width = ui.available_width();
        ui.label("LDAP server URL");
        ui.add(egui::TextEdit::singleline(&mut f.url).hint_text("ldaps://gc.example.com:3269").desired_width(width));
        ui.label("Base DN");
        ui.add(egui::TextEdit::singleline(&mut f.base_dn).hint_text("DC=company,DC=local or o=Company").desired_width(width));
        if cfg!(windows) {
            ui.checkbox(&mut f.windows_integrated, "Windows Integrated (current domain user, Negotiate + signing/sealing)");
            if f.windows_integrated {
                f.allow_insecure_anonymous = false;
                f.bind_dn.clear();
                f.password.clear();
                ui.label(RichText::new("No password required. Use ldap://server:389; your current Windows domain identity is used.").color(t.dim).small());
            }
        }
        if !f.windows_integrated {
            ui.checkbox(&mut f.allow_insecure_anonymous, "Legacy mode: unencrypted anonymous LDAP (no password)");
        }
        if f.allow_insecure_anonymous {
            ui.label(RichText::new("WARNING: names and search terms are transmitted without encryption. Only use on an isolated trusted network. No Bind DN or password is allowed.").color(t.warn).small());
            f.bind_dn.clear();
            f.password.clear();
        }
        ui.label("Bind DN (optional; own LDAP password, never mailbox password)");
        ui.add(egui::TextEdit::singleline(&mut f.bind_dn).hint_text("user@company.local or directory DN").desired_width(width));
        if !f.bind_dn.trim().is_empty() {
            ui.label("LDAP bind password (kept in keychain; enter to change)");
            ui.add(egui::TextEdit::singleline(&mut f.password).password(true).desired_width(width));
        }
        ui.label(
            RichText::new(
                "LDAP URLs use STARTTLS; LDAPS uses TLS from the start. Servers without modern TLS or LDAPv3 need a secured compatibility gateway.",
            )
            .color(t.faint)
            .small(),
        );
    }
    if let Some(e) = &f.error {
        ui.label(RichText::new(e).color(t.danger).small());
    }
    let credential_ready = f.bind_dn.trim().is_empty() || f.bind_dn.trim() == f.saved_bind_dn.trim() || !f.password.is_empty();
    let ready = !f.enabled || (!f.url.trim().is_empty() && !f.base_dn.trim().is_empty() && credential_ready);
    let (ok, cancel) = buttons(ui, t, "Save", ready);
    if ok && ready {
        let params = if f.enabled {
            json!({
                "account": f.account,
                "url": f.url.trim(),
                "baseDn": f.base_dn.trim(),
                "password": if f.password.is_empty() { None } else { Some(f.password.as_str()) },
                "bindDn": if f.bind_dn.trim().is_empty() { None } else { Some(f.bind_dn.trim()) },
                "schema": if f.schema == LdapSchema::Exchange55 { "exchange55" } else { "ad" },
                "allowInsecureAnonymous": f.allow_insecure_anonymous,
                "windowsIntegrated": f.windows_integrated
            })
        } else {
            json!({"account": f.account, "disable": true})
        };
        acts.push(Action::Submit("account.ldap", params));
    }
    !cancel
}
