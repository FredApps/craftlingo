//! Catalogs are untrusted data. Only a separately controlled approval policy can authorize export.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub type Result<T> = std::result::Result<T, String>;
const MAX_MESSAGES: usize = 10_000;
const MAX_TEXT: usize = 32_768;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub schema_version: u32,
    pub locale: String,
    pub messages: Vec<Message>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    pub id: String,
    pub source: String,
    pub description: String,
    pub translations: Vec<Translation>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Translation {
    pub scope: Scope,
    pub text: String,
    pub provenance: Provenance,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Provenance {
    Imported,
    Human,
    Ai,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq, Ord, PartialOrd)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Shared,
    Wordcraft,
    Pdfcraft,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq, Ord, PartialOrd)]
#[serde(rename_all = "lowercase")]
pub enum App {
    Wordcraft,
    Pdfcraft,
}

impl App {
    fn scope(self) -> Scope {
        match self {
            Self::Wordcraft => Scope::Wordcraft,
            Self::Pdfcraft => Scope::Pdfcraft,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub schema_version: u32,
    pub bindings: Vec<Binding>,
    pub approvals: Vec<Approval>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub app: App,
    pub id: String,
    pub source: String,
    pub context: String,
    pub allow_shared: bool,
}

/// Must live outside any translation service's write permissions.
/// Reviewer metadata alone is not authentication: the policy's repository review is the trust gate.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Approval {
    pub id: String,
    pub locale: String,
    pub scope: Scope,
    pub sha256: String,
    pub reviewed_by: String,
}

fn text_ok(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= MAX_TEXT
        && !s.chars().any(|c| c.is_control() && c != '\n' && c != '\t')
}

fn id_ok(id: &str) -> bool {
    id.len() <= 120
        && id.contains('.')
        && id.split('.').all(|part| {
            part.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
                && part
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
        })
}

fn locale_ok(locale: &str) -> bool {
    let mut parts = locale.split('-');
    let Some(language) = parts.next() else {
        return false;
    };
    locale.len() <= 35
        && (2..=8).contains(&language.len())
        && language.bytes().all(|c| c.is_ascii_alphabetic())
        && parts
            .all(|s| !s.is_empty() && s.len() <= 8 && s.bytes().all(|c| c.is_ascii_alphanumeric()))
}

/// Match the existing app parsers exactly, including repeated and empty placeholders.
pub fn placeholders(mut text: &str) -> Vec<&str> {
    let mut names = Vec::new();
    while let Some((_, rest)) = text.split_once('{') {
        let Some((name, tail)) = rest.split_once('}') else {
            break;
        };
        names.push(name);
        text = tail;
    }
    names.sort_unstable();
    names
}

/// Domain-separated, unambiguous JSON tuple binds every security-relevant translation field.
pub fn approval_digest(
    locale: &str,
    message: &Message,
    translation: &Translation,
) -> Result<String> {
    let bytes = serde_json::to_vec(&(
        "craftlingo-approval-v1",
        locale,
        &message.id,
        &message.source,
        translation.scope,
        &translation.text,
    ))
    .map_err(|e| format!("cannot encode approval tuple: {e}"))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

pub fn validate(catalog: &Catalog, policy: &Policy) -> Result<()> {
    if catalog.schema_version != 1 || policy.schema_version != 1 {
        return Err("unsupported schema version".into());
    }
    if !locale_ok(&catalog.locale) {
        return Err("invalid locale".into());
    }
    if catalog.messages.len() > MAX_MESSAGES
        || policy.bindings.len() > MAX_MESSAGES * 2
        || policy.approvals.len() > MAX_MESSAGES * 3
    {
        return Err("catalog or policy exceeds entry limit".into());
    }
    let mut messages = BTreeMap::new();
    for m in &catalog.messages {
        if !id_ok(&m.id) || !text_ok(&m.source) || !text_ok(&m.description) {
            return Err(format!("invalid identity/source/description: {}", m.id));
        }
        if messages.insert(m.id.as_str(), m).is_some() {
            return Err(format!("duplicate id: {}", m.id));
        }
        let mut scopes = BTreeSet::new();
        for t in &m.translations {
            if !scopes.insert(t.scope) {
                return Err(format!("duplicate scope: {}", m.id));
            }
            if !text_ok(&t.text)
                || placeholders(&m.source) != placeholders(&t.text)
                || m.source.ends_with('…') != t.text.ends_with('…')
            {
                return Err(format!(
                    "invalid translation, placeholders or ellipsis: {}",
                    m.id
                ));
            }
        }
    }
    let mut targets = BTreeSet::new();
    for b in &policy.bindings {
        let m = messages
            .get(b.id.as_str())
            .ok_or_else(|| format!("binding references missing id: {}", b.id))?;
        if b.source != m.source {
            return Err(format!("binding source drift: {}", b.id));
        }
        // v1 exports ordinary text only. Plurals and command-ID records need their own schema.
        if b.context.starts_with('@')
            || b.context.contains(['\n', '\r', '\t', '\\'])
            || b.context.len() > 128
            || b.context.chars().any(char::is_control)
            || (b.app == App::Wordcraft
                && !matches!(b.context.as_str(), "" | "preview" | "template"))
        {
            return Err(format!("unsupported adapter context: {}", b.id));
        }
        if !targets.insert((b.app, &b.context, &b.source)) {
            return Err(format!("duplicate legacy output key: {}", b.source));
        }
    }
    let mut approvals = BTreeSet::new();
    for a in &policy.approvals {
        if !messages.contains_key(a.id.as_str())
            || !locale_ok(&a.locale)
            || !text_ok(&a.reviewed_by)
            || a.reviewed_by.trim().is_empty()
            || a.sha256.len() != 64
            || !a
                .sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(format!("invalid trusted approval: {}", a.id));
        }
        if !approvals.insert((&a.id, &a.locale, a.scope)) {
            return Err(format!("duplicate approval: {}", a.id));
        }
    }
    Ok(())
}

pub fn escape_tsv(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
}

pub struct Export {
    pub tsv: String,
    pub exported: usize,
    pub pending: Vec<String>,
}

pub fn export(catalog: &Catalog, policy: &Policy, app: App) -> Result<Export> {
    validate(catalog, policy)?;
    let messages: BTreeMap<_, _> = catalog
        .messages
        .iter()
        .map(|m| (m.id.as_str(), m))
        .collect();
    let approved = |m: &Message, t: &Translation| -> Result<bool> {
        let digest = approval_digest(&catalog.locale, m, t)?;
        Ok(policy.approvals.iter().any(|a| {
            a.id == m.id && a.locale == catalog.locale && a.scope == t.scope && a.sha256 == digest
        }))
    };
    let mut rows = Vec::new();
    let mut pending = Vec::new();
    for b in policy.bindings.iter().filter(|b| b.app == app) {
        let m = messages
            .get(b.id.as_str())
            .ok_or_else(|| format!("missing binding: {}", b.id))?;
        let mut selected = None;
        for scope in [Some(app.scope()), b.allow_shared.then_some(Scope::Shared)]
            .into_iter()
            .flatten()
        {
            if let Some(t) = m.translations.iter().find(|t| t.scope == scope)
                && approved(m, t)?
            {
                selected = Some(t);
                break;
            }
        }
        if let Some(t) = selected {
            rows.push((b.context.clone(), b.source.clone(), t.text.clone()));
        } else {
            pending.push(b.id.clone());
        }
    }
    rows.sort();
    pending.sort();
    let mut tsv = "# CraftLingo v1 approved overlay; NOT a complete app catalog.\n".to_string();
    for (context, source, text) in &rows {
        tsv.push_str(&format!(
            "{}\t{}\t{}\n",
            escape_tsv(context),
            escape_tsv(source),
            escape_tsv(text)
        ));
    }
    Ok(Export {
        tsv,
        exported: rows.len(),
        pending,
    })
}

#[cfg(test)]
mod tests;
