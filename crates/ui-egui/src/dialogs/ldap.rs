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
    pub schema: LdapSchema,
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
            bind_dn: ldap.bind_dn.unwrap_or_default(),
            schema: ldap.schema,
            error: None,
        }
    }
}

pub(super) fn draw(ui: &mut Ui, f: &mut LdapForm, t: &Tokens, acts: &mut Vec<Action>) -> bool {
    ui.set_width(490.0);
    title(ui, t, "LDAP enterprise address book");
    ui.label(RichText::new(
        "Search your Exchange 5.5 Directory or Exchange 2003 Active Directory from recipient fields. LDAP is read-only. Connections require verified TLS."
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
        ui.label("Bind DN (optional, uses mail account password)");
        ui.add(egui::TextEdit::singleline(&mut f.bind_dn).hint_text("user@company.local or directory DN").desired_width(width));
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
    let ready = !f.enabled || ((!f.url.trim().is_empty()) && !f.base_dn.trim().is_empty());
    let (ok, cancel) = buttons(ui, t, "Save", ready);
    if ok && ready {
        let params = if f.enabled {
            json!({
                "account": f.account,
                "url": f.url.trim(),
                "baseDn": f.base_dn.trim(),
                "bindDn": if f.bind_dn.trim().is_empty() { None } else { Some(f.bind_dn.trim()) },
                "schema": if f.schema == LdapSchema::Exchange55 { "exchange55" } else { "ad" }
            })
        } else {
            json!({"account": f.account, "disable": true})
        };
        acts.push(Action::Submit("account.ldap", params));
    }
    !cancel
}
