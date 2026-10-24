//! `DataExchangeLoading` on extension methods of an object module: an interceptor or an
//! `&ИзменениеИКонтроль` method is the handler of the base method its annotation targets,
//! so it is judged by that target, never by its own name.

use ide::{Analysis, DiagnosticCode, DiagnosticsConfig};
use ide_db::base_db::{SourceDatabase, SourceRoot, SourceRootId};
use ide_db::RootDatabaseImpl;
use vfs::{FileId, FileSet, VfsPath};

const MODULE: &str = "Catalogs/Т/Ext/ObjectModule.bsl";

const GUARD: &str = "\tЕсли ОбменДанными.Загрузка Тогда\n\t\tВозврат;\n\tКонецЕсли;\n";

struct Fixture {
    analysis: Analysis,
    base_file: FileId,
    ext_file: FileId,
    _roots: tempfile::TempDir,
}

/// The base and the extension module at `rel`; without `base` the extension's base file
/// does not exist, so the pair does not resolve.
fn setup(rel: &str, base: Option<&str>, ext: &str) -> Fixture {
    let temp = tempfile::tempdir().unwrap();
    let main_root = temp.path().join("src/cf");
    let ext_root = temp.path().join("src/cfe/X");
    std::fs::create_dir_all(&main_root).unwrap();
    std::fs::create_dir_all(&ext_root).unwrap();

    let mut db = RootDatabaseImpl::new();
    db.set_all_config_paths(
        vec![(None, main_root.clone()), (Some("X".to_string()), ext_root.clone())],
        Some(bsl_metadata::ExtensionScope::InfoBase),
    );

    let base_file = FileId(0);
    let ext_file = FileId(1);
    let mut file_set = FileSet::default();
    file_set.insert(ext_file, VfsPath::new(ext_root.join(rel).to_string_lossy().as_ref()));
    if base.is_some() {
        file_set.insert(base_file, VfsPath::new(main_root.join(rel).to_string_lossy().as_ref()));
    }
    db.set_source_root(SourceRootId(0), SourceRoot::new_local(file_set));
    db.set_file_source_root(ext_file, SourceRootId(0));
    db.set_file_text(ext_file, ext);
    if let Some(base) = base {
        db.set_file_source_root(base_file, SourceRootId(0));
        db.set_file_text(base_file, base);
    }

    Fixture { analysis: Analysis::from_database(db), base_file, ext_file, _roots: temp }
}

/// The source text each `DataExchangeLoading` finding of `file` covers.
fn findings_with(fx: &Fixture, file: FileId, config: &DiagnosticsConfig) -> Vec<String> {
    let text = fx.analysis.file_text(file);
    let mut found: Vec<_> = fx
        .analysis
        .diagnostics(file, config)
        .into_iter()
        .filter(|d| d.code == DiagnosticCode::DataExchangeLoading)
        .collect();
    found.sort_by_key(|d| d.range.start());
    found.into_iter().map(|d| text[d.range].to_string()).collect()
}

fn findings(base: Option<&str>, ext: &str) -> Vec<String> {
    let fx = setup(MODULE, base, ext);
    findings_with(&fx, fx.ext_file, &DiagnosticsConfig::default())
}

fn handler(name: &str, body: &str) -> String {
    handler_with(name, "Отказ", body)
}

fn handler_with(name: &str, params: &str, body: &str) -> String {
    format!("Процедура {name}({params})\n{body}КонецПроцедуры\n")
}

fn config_with(params: serde_json::Value) -> DiagnosticsConfig {
    let mut config = DiagnosticsConfig::default();
    config.parameters.insert(DiagnosticCode::DataExchangeLoading, params);
    config
}

fn intercepted(annotation: &str, target: &str, name: &str, body: &str) -> String {
    format!("&{annotation}(\"{target}\")\n{}", handler(name, body))
}

#[test]
fn interceptor_is_judged_by_its_target_for_every_annotation_and_event() {
    for event in ["ПередЗаписью", "ПриЗаписи", "ПередУдалением"]
    {
        let base = handler(event, "\tДоработать();\n");
        for annotation in ["Перед", "После", "Вместо"] {
            for name in [format!("Расш_{event}"), "МояДоработкаЗаписи".to_string()]
            {
                let unguarded = intercepted(annotation, event, &name, "\tДоработать();\n");
                assert_eq!(
                    findings(Some(&base), &unguarded),
                    vec![name.clone()],
                    "&{annotation}(\"{event}\") {name} without a guard"
                );
                let guarded =
                    intercepted(annotation, event, &name, &format!("{GUARD}\tДоработать();\n"));
                assert!(
                    findings(Some(&base), &guarded).is_empty(),
                    "&{annotation}(\"{event}\") {name} with a guard"
                );
            }
        }
    }
}

#[test]
fn english_mixed_and_case_variant_annotations_resolve_the_target() {
    let en_base = "Procedure BeforeWrite(Cancel)\nEndProcedure\n";
    let en_ext =
        "&Before(\"BeforeWrite\")\nProcedure Ext_BeforeWrite(Cancel)\n\tDoWork();\nEndProcedure\n";
    assert_eq!(findings(Some(en_base), en_ext), ["Ext_BeforeWrite"]);
    let en_guarded = "&Before(\"BeforeWrite\")\nProcedure Ext_BeforeWrite(Cancel)\n\tIf DataExchange.Load Then\n\t\tReturn;\n\tEndIf;\nEndProcedure\n";
    assert!(findings(Some(en_base), en_guarded).is_empty());

    let base = handler("ПередЗаписью", "");
    for (annotation, target) in [("After", "ПередЗаписью"), ("ПОСЛЕ", "передзаписью")]
    {
        let ext = intercepted(annotation, target, "Расш_ПередЗаписью", "\tДоработать();\n");
        assert_eq!(findings(Some(&base), &ext), ["Расш_ПередЗаписью"], "&{annotation}");
    }
}

#[test]
fn guard_in_the_base_does_not_cover_the_interceptor() {
    let base = handler("ПередЗаписью", GUARD);
    let ext = intercepted("После", "ПередЗаписью", "Расш_ПередЗаписью", "\tДоработать();\n");
    assert_eq!(findings(Some(&base), &ext), ["Расш_ПередЗаписью"]);
}

#[test]
fn an_unproven_event_gives_no_finding_whatever_the_own_name() {
    let body = "\tДоработать();\n";
    let base =
        format!("{}\n{}", handler("ПриЗаписи", ""), handler("ОбработкаПроверкиЗаполнения", ""));
    // Positive control on the same base: the target is a monitored handler.
    let control = intercepted("После", "ПриЗаписи", "ПередЗаписью", body);
    assert_eq!(findings(Some(&base), &control), ["ПередЗаписью"]);

    let unmonitored = intercepted("После", "ОбработкаПроверкиЗаполнения", "ПередЗаписью", body);
    assert!(findings(Some(&base), &unmonitored).is_empty(), "unmonitored target");
    let absent = intercepted("После", "ПередЗаписью", "ПередЗаписью", body);
    assert!(findings(Some(&base), &absent).is_empty(), "target absent from the base");
    let function_base = "Функция ПередЗаписью(Отказ)\n\tВозврат Истина;\nКонецФункции\n";
    assert!(findings(Some(function_base), &absent).is_empty(), "target is a function");
    assert!(findings(None, &control).is_empty(), "no base file");
    for annotation in ["&После()", "&После(ПриЗаписи)"] {
        let unresolved = format!("{annotation}\n{}", handler("ПередЗаписью", body));
        assert!(findings(Some(&base), &unresolved).is_empty(), "{annotation}: no string target");
    }
}

#[test]
fn extension_methods_are_judged_only_in_handler_module_kinds() {
    let base = handler("ПередЗаписью", "\tДоработать();\n");
    let interceptor =
        intercepted("После", "ПередЗаписью", "Расш_ПередЗаписью", "\tДоработать();\n");
    let change_and_validate = intercepted(
        "ИзменениеИКонтроль",
        "ПередЗаписью",
        "Расш1_ПередЗаписью",
        "\tДоработать();\n",
    );
    for (rel, judged) in [
        ("Catalogs/Т/Ext/ObjectModule.bsl", true),
        ("InformationRegisters/Р/Ext/RecordSetModule.bsl", true),
        ("Constants/К/Ext/ValueManagerModule.bsl", true),
        ("Catalogs/Т/Ext/ManagerModule.bsl", false),
        ("CommonModules/М/Ext/Module.bsl", false),
    ] {
        for (ext, name) in
            [(&interceptor, "Расш_ПередЗаписью"), (&change_and_validate, "Расш1_ПередЗаписью")]
        {
            let fx = setup(rel, Some(&base), ext);
            let found = findings_with(&fx, fx.ext_file, &DiagnosticsConfig::default());
            let expected: &[&str] = if judged { &[name] } else { &[] };
            assert_eq!(found, expected, "{rel}: {name}");
        }
    }
}

#[test]
fn interceptor_findings_do_not_depend_on_the_weaving_diagnostics() {
    let base = handler("ПередЗаписью", "");
    let ext = intercepted("После", "ПередЗаписью", "Расш_ПередЗаписью", "\tДоработать();\n");
    let fx = setup(MODULE, Some(&base), &ext);
    let config = DiagnosticsConfig {
        disabled: vec![
            DiagnosticCode::WeavingSignatureMismatch,
            DiagnosticCode::WeavingAnnotationNotApplicable,
        ],
        ..DiagnosticsConfig::default()
    };
    assert_eq!(findings_with(&fx, fx.ext_file, &config), ["Расш_ПередЗаписью"]);

    let disabled = DiagnosticsConfig {
        disabled: vec![DiagnosticCode::DataExchangeLoading],
        ..DiagnosticsConfig::default()
    };
    assert!(findings_with(&fx, fx.ext_file, &disabled).is_empty());
}

/// The extension text after applying the only fix of the only `DataExchangeLoading` finding.
fn with_guard_fix_applied(base: &str, ext: &str) -> String {
    let fx = setup(MODULE, Some(base), ext);
    let diagnostics = fx.analysis.diagnostics(fx.ext_file, &DiagnosticsConfig::default());
    let found: Vec<_> =
        diagnostics.iter().filter(|d| d.code == DiagnosticCode::DataExchangeLoading).collect();
    let [finding] = found.as_slice() else { panic!("one finding: {found:?}") };
    let [fix] = finding.fixes.as_slice() else { panic!("one fix: {:?}", finding.fixes) };
    let [edit] = fix.edits.as_slice() else { panic!("one edit: {:?}", fix.edits) };
    let mut fixed = ext.to_string();
    fixed.insert_str(edit.range.start().into(), &edit.new_text);
    fixed
}

#[test]
fn interceptor_fix_inserts_the_guard_into_its_own_body() {
    let base = handler("ПередЗаписью", "");
    for annotation in ["Перед", "После", "Вместо"] {
        let ext = intercepted(annotation, "ПередЗаписью", "Расш_ПередЗаписью", "\tДоработать();\n");
        assert_eq!(
            with_guard_fix_applied(&base, &ext),
            intercepted(
                annotation,
                "ПередЗаписью",
                "Расш_ПередЗаписью",
                &format!("{GUARD}\tДоработать();\n")
            ),
            "&{annotation}"
        );
    }
}

#[test]
fn english_interceptor_gets_the_english_guard() {
    let base = "Procedure BeforeWrite(Cancel)\nEndProcedure\n";
    let ext =
        "&After(\"BeforeWrite\")\nProcedure Ext_BeforeWrite(Cancel)\n\tDoWork();\nEndProcedure\n";
    assert_eq!(
        with_guard_fix_applied(base, ext),
        "&After(\"BeforeWrite\")\nProcedure Ext_BeforeWrite(Cancel)\n\tIf DataExchange.Load Then\n\t\tReturn;\n\tEndIf;\n\tDoWork();\nEndProcedure\n"
    );
}

#[test]
fn the_base_handler_is_still_judged_on_its_own() {
    let base = handler("ПередЗаписью", "\tДоработать();\n");
    let ext = intercepted("После", "ПередЗаписью", "Расш_ПередЗаписью", GUARD);
    let fx = setup(MODULE, Some(&base), &ext);
    assert_eq!(findings_with(&fx, fx.base_file, &DiagnosticsConfig::default()), ["ПередЗаписью"]);
}

#[test]
fn change_and_validate_is_judged_by_its_final_body() {
    let work = "\tДоработать();\n";
    for event in ["ПередЗаписью", "ПриЗаписи", "ПередУдалением"]
    {
        let name = format!("Расш1_{event}");
        let guarded_base = handler(event, &format!("{GUARD}{work}"));
        let plain_base = handler(event, work);
        let cav = |body: &str| intercepted("ИзменениеИКонтроль", event, &name, body);

        let copied_guard = cav(&format!("{GUARD}{work}"));
        assert!(findings(Some(&guarded_base), &copied_guard).is_empty(), "{event}: guard copied");

        let copied_plain = cav(work);
        assert_eq!(
            findings(Some(&plain_base), &copied_plain),
            std::slice::from_ref(&name),
            "{event}: no guard"
        );

        let deleted = cav(&format!("#Удаление\n{GUARD}#КонецУдаления\n{work}"));
        assert_eq!(
            findings(Some(&guarded_base), &deleted),
            std::slice::from_ref(&name),
            "{event}: deleted"
        );

        let inserted = cav(&format!("#Вставка\n{GUARD}#КонецВставки\n{work}"));
        assert!(findings(Some(&plain_base), &inserted).is_empty(), "{event}: inserted guard");

        let inserted_plain = cav(&format!("#Вставка\n\tЕщё();\n#КонецВставки\n{work}"));
        assert_eq!(
            findings(Some(&plain_base), &inserted_plain),
            std::slice::from_ref(&name),
            "{event}: insertion without a guard"
        );
    }
}

#[test]
fn change_and_validate_finding_has_no_fix() {
    let base = handler("ПередЗаписью", GUARD);
    let ext = intercepted(
        "ИзменениеИКонтроль",
        "ПередЗаписью",
        "Расш1_ПередЗаписью",
        &format!("#Удаление\n{GUARD}#КонецУдаления\n"),
    );
    let fx = setup(MODULE, Some(&base), &ext);
    let diagnostics = fx.analysis.diagnostics(fx.ext_file, &DiagnosticsConfig::default());
    let found: Vec<_> =
        diagnostics.iter().filter(|d| d.code == DiagnosticCode::DataExchangeLoading).collect();
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].fixes.is_empty(), "{:?}", found[0].fixes);
}

#[test]
fn change_and_validate_english_case_and_own_monitored_name() {
    let en_base = "Procedure BeforeWrite(Cancel)\n\tDoWork();\nEndProcedure\n";
    let en_ext = "&ChangeAndValidate(\"BeforeWrite\")\nProcedure Ext1_BeforeWrite(Cancel)\n\tDoWork();\nEndProcedure\n";
    assert_eq!(findings(Some(en_base), en_ext), ["Ext1_BeforeWrite"]);

    let base = handler("ПередЗаписью", "\tДоработать();\n");
    let lower = intercepted(
        "изменениеиконтроль",
        "передзаписью",
        "Расш1_ПередЗаписью",
        "\tДоработать();\n",
    );
    assert_eq!(findings(Some(&base), &lower), ["Расш1_ПередЗаписью"]);

    let own_name_inserted = intercepted(
        "ИзменениеИКонтроль",
        "ПередЗаписью",
        "ПередЗаписью",
        &format!("#Вставка\n{GUARD}#КонецВставки\n\tДоработать();\n"),
    );
    assert!(findings(Some(&base), &own_name_inserted).is_empty());
    let own_name_plain =
        intercepted("ИзменениеИКонтроль", "ПередЗаписью", "ПередЗаписью", "\tДоработать();\n");
    assert_eq!(findings(Some(&base), &own_name_plain), ["ПередЗаписью"]);
}

#[test]
fn change_and_validate_without_a_proven_target_gives_no_finding() {
    let work = "\tДоработать();\n";
    let base = format!(
        "{}\n{}",
        handler("ПередЗаписью", work),
        handler("ОбработкаПроверкиЗаполнения", work)
    );
    let cav =
        |target: &str, body: &str| intercepted("ИзменениеИКонтроль", target, "Расш1_Метод", body);
    // Positive control on the same base.
    assert_eq!(findings(Some(&base), &cav("ПередЗаписью", work)), ["Расш1_Метод"]);

    assert!(findings(None, &cav("ПередЗаписью", work)).is_empty(), "no base file");
    assert!(findings(Some(&base), &cav("ПриЗаписи", work)).is_empty(), "target absent");
    assert!(
        findings(Some(&base), &cav("ПередЗаписью", &format!("#Вставка\n{work}"))).is_empty(),
        "unbalanced markers"
    );
    assert!(
        findings(Some(&base), &cav("ОбработкаПроверкиЗаполнения", work)).is_empty(),
        "unmonitored target"
    );
    let function_base = "Функция ПередЗаписью(Отказ)\n\tВозврат Истина;\nКонецФункции\n";
    assert!(findings(Some(function_base), &cav("ПередЗаписью", work)).is_empty(), "function");
}

/// The merge splices only the first `&ИзменениеИКонтроль` method of a target; a later
/// one has no final body in the effective module, so it is not judged by the first's.
#[test]
fn only_the_spliced_change_and_validate_method_of_a_target_is_judged() {
    let work = "\tДоработать();\n";
    let base = handler("ПередЗаписью", &format!("{GUARD}{work}"));
    let first = intercepted("ИзменениеИКонтроль", "ПередЗаписью", "Расш1_ПередЗаписью", work);
    let second = intercepted(
        "ИзменениеИКонтроль",
        "ПередЗаписью",
        "Расш2_ПередЗаписью",
        &format!("{GUARD}{work}"),
    );
    assert_eq!(findings(Some(&base), &format!("{first}{second}")), ["Расш1_ПередЗаписью"]);
    assert!(findings(Some(&base), &format!("{second}{first}")).is_empty());
}

#[test]
fn around_interceptor_proceeding_to_the_base_needs_its_own_guard() {
    let params = "Отказ, РежимЗаписи, РежимПроведения";
    let proceed = "\tПродолжитьВызов(Отказ, РежимЗаписи, РежимПроведения);\n";
    // The base is guarded: its guard does not cover the code `&Вместо` runs around it.
    let base = handler_with("ПередЗаписью", params, &format!("{GUARD}\tДоработать();\n"));
    let around = |body: &str| {
        format!("&Вместо(\"ПередЗаписью\")\n{}", handler_with("Расш_ПередЗаписью", params, body))
    };
    assert_eq!(findings(Some(&base), &around(proceed)), ["Расш_ПередЗаписью"]);
    assert!(findings(Some(&base), &around(&format!("{GUARD}{proceed}"))).is_empty());
}

#[test]
fn find_first_setting_applies_to_interceptors() {
    let base = handler("ПередЗаписью", "");
    let late = intercepted(
        "После",
        "ПередЗаписью",
        "Расш_ПередЗаписью",
        &format!("\tПодготовить();\n{GUARD}"),
    );
    let early = intercepted(
        "После",
        "ПередЗаписью",
        "Расш_ПередЗаписью",
        &format!("{GUARD}\tПодготовить();\n"),
    );
    let find_first = config_with(serde_json::json!({"findFirst": true}));

    let fx = setup(MODULE, Some(&base), &late);
    assert!(
        findings_with(&fx, fx.ext_file, &DiagnosticsConfig::default()).is_empty(),
        "a guard anywhere counts by default"
    );
    assert_eq!(findings_with(&fx, fx.ext_file, &find_first), ["Расш_ПередЗаписью"]);

    let fx = setup(MODULE, Some(&base), &early);
    assert!(findings_with(&fx, fx.ext_file, &find_first).is_empty(), "the guard comes first");
}

#[test]
fn guard_wrappers_setting_applies_to_interceptors() {
    let base = handler("ПередЗаписью", "");
    let wrapped = |wrapper: &str| {
        intercepted(
            "После",
            "ПередЗаписью",
            "Расш_ПередЗаписью",
            &format!(
                "\tЕсли {wrapper}(ЭтотОбъект) Тогда\n\t\tВозврат;\n\tКонецЕсли;\n\tДоработать();\n"
            ),
        )
    };
    let custom = config_with(serde_json::json!({"guardWrappers": ["Обмен.ЭтоЗагрузка"]}));
    let default_wrapper = wrapped("ЗарплатаКадры.ОтключитьБизнесЛогикуПриЗаписи");
    let custom_wrapper = wrapped("Обмен.ЭтоЗагрузка");

    let fx = setup(MODULE, Some(&base), &default_wrapper);
    assert!(findings_with(&fx, fx.ext_file, &DiagnosticsConfig::default()).is_empty());
    assert_eq!(
        findings_with(&fx, fx.ext_file, &custom),
        ["Расш_ПередЗаписью"],
        "the configured list replaces the default"
    );

    let fx = setup(MODULE, Some(&base), &custom_wrapper);
    assert!(findings_with(&fx, fx.ext_file, &custom).is_empty());
    assert_eq!(
        findings_with(&fx, fx.ext_file, &DiagnosticsConfig::default()),
        ["Расш_ПередЗаписью"],
        "the custom wrapper is not a default one"
    );
}

#[test]
fn each_interceptor_gets_one_finding_on_its_own_declaration() {
    let work = "\tДоработать();\n";
    let base = format!(
        "{}\n{}\n{}",
        handler("ПередЗаписью", ""),
        handler("ПриЗаписи", ""),
        handler("ПередУдалением", "")
    );
    let before = intercepted("Перед", "ПередЗаписью", "Расш1_ПередЗаписью", work);
    let after = intercepted("После", "ПередЗаписью", "Расш2_ПередЗаписью", work);
    let guarded_after =
        intercepted("После", "ПередЗаписью", "Расш2_ПередЗаписью", &format!("{GUARD}{work}"));
    assert_eq!(
        findings(Some(&base), &format!("{before}\n{after}")),
        ["Расш1_ПередЗаписью", "Расш2_ПередЗаписью"]
    );
    assert_eq!(
        findings(Some(&base), &format!("{before}\n{guarded_after}")),
        ["Расш1_ПередЗаписью"]
    );

    // Own names are controlled handler names, targets are other handlers: each declaration
    // is judged once, by its target.
    let own_a = intercepted("После", "ПриЗаписи", "ПередЗаписью", work);
    let own_b = intercepted("Перед", "ПередУдалением", "OnWrite", work);
    assert_eq!(findings(Some(&base), &format!("{own_a}\n{own_b}")), ["ПередЗаписью", "OnWrite"]);
}

#[test]
fn a_shorter_signature_does_not_cancel_the_finding_and_a_function_is_not_judged() {
    let base =
        handler_with("ПередЗаписью", "Отказ, РежимЗаписи, РежимПроведения", "\tДоработать();\n");
    let shorter = intercepted("После", "ПередЗаписью", "Расш_ПередЗаписью", "\tДоработать();\n");
    assert_eq!(findings(Some(&base), &shorter), ["Расш_ПередЗаписью"]);

    let function = "&После(\"ПередЗаписью\")\nФункция Расш_ПередЗаписью(Отказ)\n\tДоработать();\n\tВозврат Неопределено;\nКонецФункции\n";
    assert!(findings(Some(&base), function).is_empty());
}

#[test]
fn annotated_method_of_a_base_file_gives_no_finding() {
    let guarded_on_write = handler("ПриЗаписи", GUARD);
    let plain = format!("{guarded_on_write}\n{}", handler("ПередЗаписью", "\tДоработать();\n"));
    let fx = setup(MODULE, Some(&plain), "");
    assert_eq!(
        findings_with(&fx, fx.base_file, &DiagnosticsConfig::default()),
        ["ПередЗаписью"],
        "the handler itself is judged"
    );

    let annotated = format!(
        "{guarded_on_write}\n{}",
        intercepted("После", "ПриЗаписи", "ПередЗаписью", "\tДоработать();\n")
    );
    let fx = setup(MODULE, Some(&annotated), "");
    assert!(findings_with(&fx, fx.base_file, &DiagnosticsConfig::default()).is_empty());
}

#[test]
fn change_and_validate_follows_the_disabled_setting() {
    let base = handler("ПередЗаписью", GUARD);
    let ext = intercepted(
        "ИзменениеИКонтроль",
        "ПередЗаписью",
        "Расш1_ПередЗаписью",
        &format!("#Удаление\n{GUARD}#КонецУдаления\n"),
    );
    let fx = setup(MODULE, Some(&base), &ext);
    assert_eq!(
        findings_with(&fx, fx.ext_file, &DiagnosticsConfig::default()),
        ["Расш1_ПередЗаписью"]
    );
    let disabled = DiagnosticsConfig {
        disabled: vec![DiagnosticCode::DataExchangeLoading],
        ..DiagnosticsConfig::default()
    };
    assert!(findings_with(&fx, fx.ext_file, &disabled).is_empty());
}

#[test]
fn change_and_validate_english_markers_and_lowercase_follow_the_same_rules() {
    let en_guard = "\tIf DataExchange.Load Then\n\t\tReturn;\n\tEndIf;\n";
    let en_work = "\tDoWork();\n";
    let en_guarded = format!("Procedure BeforeWrite(Cancel)\n{en_guard}{en_work}EndProcedure\n");
    let en_plain = format!("Procedure BeforeWrite(Cancel)\n{en_work}EndProcedure\n");
    let en = |body: &str| {
        format!(
            "&ChangeAndValidate(\"BeforeWrite\")\nProcedure Ext1_BeforeWrite(Cancel)\n{body}EndProcedure\n"
        )
    };
    assert!(findings(Some(&en_guarded), &en(&format!("{en_guard}{en_work}"))).is_empty());
    assert_eq!(
        findings(Some(&en_guarded), &en(&format!("#Delete\n{en_guard}#EndDelete\n{en_work}"))),
        ["Ext1_BeforeWrite"]
    );
    assert!(findings(Some(&en_plain), &en(&format!("#Insert\n{en_guard}#EndInsert\n{en_work}")))
        .is_empty());
    assert_eq!(
        findings(Some(&en_plain), &en(&format!("#Insert\n\tAlso();\n#EndInsert\n{en_work}"))),
        ["Ext1_BeforeWrite"]
    );

    let work = "\tДоработать();\n";
    let guarded = handler("ПередЗаписью", &format!("{GUARD}{work}"));
    let plain = handler("ПередЗаписью", work);
    let lower =
        |body: &str| intercepted("изменениеиконтроль", "передзаписью", "Расш1_ПередЗаписью", body);
    assert!(findings(Some(&guarded), &lower(&format!("{GUARD}{work}"))).is_empty());
    assert_eq!(
        findings(Some(&guarded), &lower(&format!("#удаление\n{GUARD}#конецудаления\n{work}"))),
        ["Расш1_ПередЗаписью"]
    );
    assert!(findings(Some(&plain), &lower(&format!("#вставка\n{GUARD}#конецвставки\n{work}")))
        .is_empty());
    assert_eq!(findings(Some(&plain), &lower(work)), ["Расш1_ПередЗаписью"]);
}

/// Another method keeps the merge alive, so the base handler's own body is in the effective
/// module; the method whose markers are unbalanced is not spliced there and must not be
/// judged by that body.
#[test]
fn change_and_validate_left_out_of_a_live_merge_gives_no_finding() {
    let work = "\tДоработать();\n";
    let base = format!(
        "{}\n{}",
        handler("ПередЗаписью", work),
        handler("ПриЗаписи", &format!("{GUARD}{work}"))
    );
    let cav = |target: &str, name: &str, body: &str| {
        intercepted("ИзменениеИКонтроль", target, name, body)
    };
    let live = cav("ПриЗаписи", "Расш1_ПриЗаписи", &format!("{GUARD}{work}"));
    let spliced = cav("ПередЗаписью", "Расш1_ПередЗаписью", work);
    assert_eq!(
        findings(Some(&base), &format!("{spliced}\n{live}")),
        ["Расш1_ПередЗаписью"],
        "control: spliced into the live merge"
    );

    let unbalanced = cav("ПередЗаписью", "Расш1_ПередЗаписью", &format!("#Вставка\n{work}"));
    assert!(findings(Some(&base), &format!("{unbalanced}\n{live}")).is_empty());
}
