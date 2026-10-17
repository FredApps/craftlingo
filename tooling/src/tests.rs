use super::*;

fn fixture() -> (Catalog, Policy) {
    let catalog = Catalog {
        schema_version: 1,
        locale: "es".into(),
        messages: vec![Message {
            id: "common.save".into(),
            source: "Save".into(),
            description: "Save the current document".into(),
            translations: vec![Translation {
                scope: Scope::Shared,
                text: "Guardar".into(),
                provenance: Provenance::Imported,
            }],
        }],
    };
    let policy = Policy {
        schema_version: 1,
        bindings: vec![Binding {
            app: App::Wordcraft,
            id: "common.save".into(),
            source: "Save".into(),
            context: "".into(),
            allow_shared: true,
        }],
        approvals: vec![],
    };
    (catalog, policy)
}

fn approve(c: &Catalog, p: &mut Policy, scope: Scope) {
    let m = &c.messages[0];
    let t = m.translations.iter().find(|t| t.scope == scope).unwrap();
    p.approvals
        .retain(|a| !(a.id == m.id && a.locale == c.locale && a.scope == scope));
    p.approvals.push(Approval {
        id: m.id.clone(),
        locale: c.locale.clone(),
        scope,
        sha256: approval_digest(&c.locale, m, t).unwrap(),
        reviewed_by: "test-fixture-reviewer".into(),
    });
}

#[test]
fn imported_human_or_ai_provenance_cannot_approve_itself() {
    for provenance in [Provenance::Imported, Provenance::Human, Provenance::Ai] {
        let (mut c, p) = fixture();
        c.messages[0].translations[0].provenance = provenance;
        let e = export(&c, &p, App::Wordcraft).unwrap();
        assert_eq!(e.exported, 0);
        assert_eq!(e.pending, ["common.save"]);
        assert!(!e.tsv.contains("Guardar"));
    }
}

#[test]
fn exact_trusted_approval_exports_existing_plain_tsv_shape() {
    let (c, mut p) = fixture();
    approve(&c, &mut p, Scope::Shared);
    let e = export(&c, &p, App::Wordcraft).unwrap();
    assert_eq!(e.exported, 1);
    assert!(e.pending.is_empty());
    assert!(e.tsv.ends_with("\tSave\tGuardar\n"));
}

#[test]
fn changed_translation_cannot_reuse_approval() {
    let (mut c, mut p) = fixture();
    approve(&c, &mut p, Scope::Shared);
    c.messages[0].translations[0].text = "Changed".into();
    assert_eq!(export(&c, &p, App::Wordcraft).unwrap().exported, 0);
}

#[test]
fn changed_source_cannot_reuse_approval_even_with_updated_binding() {
    let (mut c, mut p) = fixture();
    approve(&c, &mut p, Scope::Shared);
    c.messages[0].source = "Save document".into();
    p.bindings[0].source = "Save document".into();
    assert_eq!(export(&c, &p, App::Wordcraft).unwrap().exported, 0);
}

#[test]
fn approval_binds_locale_identity_and_scope() {
    let (mut c, mut p) = fixture();
    approve(&c, &mut p, Scope::Shared);
    c.locale = "es-MX".into();
    assert_eq!(export(&c, &p, App::Wordcraft).unwrap().exported, 0);
    c.locale = "es".into();
    c.messages[0].translations[0].scope = Scope::Wordcraft;
    assert_eq!(export(&c, &p, App::Wordcraft).unwrap().exported, 0);
    c.messages[0].translations[0].scope = Scope::Shared;
    let old = approval_digest(&c.locale, &c.messages[0], &c.messages[0].translations[0]).unwrap();
    c.messages[0].id = "common.save_as".into();
    assert_ne!(
        old,
        approval_digest(&c.locale, &c.messages[0], &c.messages[0].translations[0]).unwrap()
    );
}

#[test]
fn approved_app_override_wins_without_leaking_to_other_app() {
    let (mut c, mut p) = fixture();
    p.bindings.push(Binding {
        app: App::Pdfcraft,
        ..p.bindings[0].clone()
    });
    c.messages[0].translations.push(Translation {
        scope: Scope::Wordcraft,
        text: "Guardar documento".into(),
        provenance: Provenance::Human,
    });
    approve(&c, &mut p, Scope::Shared);
    approve(&c, &mut p, Scope::Wordcraft);
    assert!(
        export(&c, &p, App::Wordcraft)
            .unwrap()
            .tsv
            .contains("Guardar documento")
    );
    let pdf = export(&c, &p, App::Pdfcraft).unwrap();
    assert!(pdf.tsv.ends_with("\tSave\tGuardar\n"));
}

#[test]
fn unapproved_override_does_not_replace_approved_shared_value() {
    let (mut c, mut p) = fixture();
    c.messages[0].translations.push(Translation {
        scope: Scope::Wordcraft,
        text: "Suggestion".into(),
        provenance: Provenance::Ai,
    });
    approve(&c, &mut p, Scope::Shared);
    assert!(
        export(&c, &p, App::Wordcraft)
            .unwrap()
            .tsv
            .ends_with("\tSave\tGuardar\n")
    );
    p.bindings[0].allow_shared = false;
    assert_eq!(export(&c, &p, App::Wordcraft).unwrap().exported, 0);
}

#[test]
fn placeholders_match_app_multiset_and_permissive_lexer() {
    assert_eq!(
        placeholders("{x} {x} {} {{name}} } {unfinished"),
        ["", "x", "x", "{name"]
    );
    let (mut c, mut p) = fixture();
    c.messages[0].source = "{x} {x}".into();
    p.bindings[0].source = c.messages[0].source.clone();
    c.messages[0].translations[0].text = "{x}".into();
    assert!(validate(&c, &p).is_err());
    c.messages[0].translations[0].text = "{x} y {x}".into();
    assert!(validate(&c, &p).is_ok());
}

#[test]
fn exact_unicode_ellipsis_is_required_without_trimming() {
    let (mut c, mut p) = fixture();
    c.messages[0].source = "Browse…".into();
    p.bindings[0].source = "Browse…".into();
    for bad in ["Examinar...", "Examinar… ", "Examinar"] {
        c.messages[0].translations[0].text = bad.into();
        assert!(validate(&c, &p).is_err());
    }
    c.messages[0].translations[0].text = "Examinar…".into();
    assert!(validate(&c, &p).is_ok());
}

#[test]
fn tsv_escaping_is_lossless_for_supported_specials() {
    assert_eq!(escape_tsv("a\\b\nc\td"), "a\\\\b\\nc\\td");
    let (mut c, mut p) = fixture();
    c.messages[0].source = "A\nB\tC\\D".into();
    p.bindings[0].source = c.messages[0].source.clone();
    c.messages[0].translations[0].text = "E\nF\tG\\H".into();
    approve(&c, &mut p, Scope::Shared);
    assert!(
        export(&c, &p, App::Wordcraft)
            .unwrap()
            .tsv
            .ends_with("\tA\\nB\\tC\\\\D\tE\\nF\\tG\\\\H\n")
    );
}

#[test]
fn duplicate_ids_scopes_and_legacy_keys_are_rejected() {
    let (mut c, mut p) = fixture();
    c.messages.push(c.messages[0].clone());
    assert!(validate(&c, &p).is_err());
    c.messages.pop();
    let duplicate = c.messages[0].translations[0].clone();
    c.messages[0].translations.push(duplicate);
    assert!(validate(&c, &p).is_err());
    c.messages[0].translations.pop();
    p.bindings.push(p.bindings[0].clone());
    assert!(validate(&c, &p).is_err());
}

#[test]
fn unsupported_plural_and_context_records_fail_closed() {
    let (c, mut p) = fixture();
    for context in ["@plural", "@id", "arbitrary", "bad\tcontext"] {
        p.bindings[0].context = context.into();
        assert!(validate(&c, &p).is_err());
    }
    p.bindings[0].context = "preview".into();
    assert!(validate(&c, &p).is_ok());
    p.bindings[0].app = App::Pdfcraft;
    p.bindings[0].context = "key".into();
    assert!(validate(&c, &p).is_ok());
}

#[test]
fn unknown_catalog_fields_cannot_smuggle_approval() {
    let (c, _) = fixture();
    let mut value = serde_json::to_value(c).unwrap();
    value["approved"] = true.into();
    assert!(serde_json::from_value::<Catalog>(value).is_err());
}

#[test]
fn malformed_versions_controls_and_binding_drift_fail() {
    let (mut c, mut p) = fixture();
    c.schema_version = 2;
    assert!(validate(&c, &p).is_err());
    c.schema_version = 1;
    c.messages[0].translations[0].text = "bad\rtext".into();
    assert!(validate(&c, &p).is_err());
    c.messages[0].translations[0].text = "Guardar".into();
    p.bindings[0].source = "Other".into();
    assert!(validate(&c, &p).is_err());
}

#[test]
fn deterministic_output_ignores_input_order() {
    let (mut c, mut p) = fixture();
    approve(&c, &mut p, Scope::Shared);
    c.messages.push(Message {
        id: "common.cancel".into(),
        source: "Cancel".into(),
        description: "Cancel the pending operation".into(),
        translations: vec![Translation {
            scope: Scope::Shared,
            text: "Cancelar".into(),
            provenance: Provenance::Human,
        }],
    });
    p.bindings.push(Binding {
        id: "common.cancel".into(),
        source: "Cancel".into(),
        ..p.bindings[0].clone()
    });
    let m = &c.messages[1];
    p.approvals.push(Approval {
        id: m.id.clone(),
        locale: c.locale.clone(),
        scope: Scope::Shared,
        sha256: approval_digest(&c.locale, m, &m.translations[0]).unwrap(),
        reviewed_by: "test-fixture-reviewer".into(),
    });
    let first = export(&c, &p, App::Wordcraft).unwrap().tsv;
    c.messages.reverse();
    p.bindings.reverse();
    p.approvals.reverse();
    assert_eq!(first, export(&c, &p, App::Wordcraft).unwrap().tsv);
}
