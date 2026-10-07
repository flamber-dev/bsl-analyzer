use crate::define_metadata;
use crate::metadata::*;
use crate::{AnalysisContext, BodyContext};
use crate::{Diagnostic, DiagnosticCode};
use bsl_platform::PlatformVersion;
use hir::LocalRange;
use hir::Name;
use syntax::SyntaxKind;

pub const METADATA: DiagnosticMetadata = define_metadata! {
    diagnostic_type: DiagnosticType::Error,
    severity: DiagnosticSeverityLevel::Major,
    scope: DiagnosticScope::All,
    modules: &[],
    minutes_to_fix: 10,
    activated_by_default: true,
    compatibility_mode: DiagnosticCompatibilityMode::Undefined,
    tags: &[MetadataTag::Error, MetadataTag::Suspicious],
    can_locate_on_project: false,
    extra_min_for_complexity: 0.0,
    lsp_severity_override: "",
};

fn message(
    name: &str,
    introduced: PlatformVersion,
    minimum: PlatformVersion,
    ctx: &AnalysisContext,
) -> String {
    match ctx.locale() {
        base_db::Locale::Ru => format!(
            "'{name}' доступен с версии платформы {introduced}, а минимальная платформа проекта {minimum}: на ней этого ещё нет"
        ),
        base_db::Locale::En => format!(
            "'{name}' is available since platform {introduced}, but the project's minimum platform is {minimum}: that platform does not have it yet"
        ),
    }
}

/// A platform member resolved by inference (global function or property, type,
/// method or property of a platform-typed value).
pub fn from_hir(
    name: &Name,
    introduced: PlatformVersion,
    minimum: PlatformVersion,
    range: LocalRange,
    ctx: &AnalysisContext,
) -> Option<Diagnostic<LocalRange>> {
    crate::simple_hir_diagnostic(
        DiagnosticCode::PlatformMemberNewerThanMinVersion,
        message(name.as_str(), introduced, minimum, ctx),
        range,
        ctx,
    )
}

/// The language part, which no member lookup sees: an `Асинх` method declaration
/// and a `Ждать` operator. Both are syntax, so they are judged here, on the
/// tokens of the body, and dated [`hir::min_platform::ASYNC_INTRODUCED`].
pub fn check_body(ctx: &BodyContext, acc: &mut Vec<Diagnostic<LocalRange>>) {
    let code = DiagnosticCode::PlatformMemberNewerThanMinVersion;
    if ctx.is_disabled_with_metadata(code) {
        return;
    }
    let Some(minimum) = ctx.min_platform_version() else {
        return;
    };
    let introduced = hir::min_platform::ASYNC_INTRODUCED;
    if !introduced.release_newer_than(minimum) {
        return;
    }
    for token in ctx.tokens() {
        let Some(parent) = token.parent() else {
            continue;
        };
        let owned = match token.kind() {
            SyntaxKind::KW_ASYNC => crate::body_context::is_method_node(&parent),
            SyntaxKind::KW_AWAIT => parent.kind() == SyntaxKind::AWAIT_EXPR,
            _ => false,
        };
        if !owned {
            continue;
        }
        if let Some(diagnostic) = crate::simple_hir_diagnostic(
            code,
            message(token.text(), introduced, minimum, ctx),
            ctx.token_range(&token),
            ctx,
        ) {
            acc.push(diagnostic);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Diagnostic, DiagnosticCode, DiagnosticsConfig};

    fn run(
        source: &str,
        minimum: Option<&str>,
        builder: test_fixture::CfeFixtureBuilder,
    ) -> Vec<Diagnostic> {
        let minimum = minimum.map(std::sync::Arc::<str>::from);
        crate::test_utils::check_cfe_at_with_db_setup(
            "CommonModules/Caller/Ext/Module.bsl",
            source,
            builder.build(),
            &[],
            DiagnosticsConfig::default(),
            |_| {},
            |db| db.set_min_platform_version(minimum),
            |db, ctx| crate::file_diagnostics(db, ctx.file_id, ctx.config),
        )
        .into_iter()
        .filter(|diag| diag.code == DiagnosticCode::PlatformMemberNewerThanMinVersion)
        .collect()
    }

    fn newer(source: &str, minimum: Option<&str>) -> Vec<String> {
        run(source, minimum, test_fixture::CfeFixtureBuilder::new(""))
            .into_iter()
            .map(|diag| diag.message)
            .collect()
    }

    #[test]
    fn global_function_newer_than_the_minimum_is_reported() {
        let source = r#"
Процедура Тест()
    Результат = СтрЗаменитьПоРегулярномуВыражению("abc", "b", "x");
КонецПроцедуры
"#;
        let reported = newer(source, Some("8.3.17"));
        assert_eq!(reported.len(), 1, "{reported:?}");
        assert!(reported[0].contains("СтрЗаменитьПоРегулярномуВыражению"), "{reported:?}");
        assert!(reported[0].contains("8.3.23"), "{reported:?}");
        assert!(reported[0].contains("8.3.17"), "{reported:?}");

        assert!(newer(source, Some("8.3.23")).is_empty(), "8.3.23 has the function");
    }

    #[test]
    fn global_functions_old_enough_stay_silent() {
        let source = r#"
Процедура Тест()
    Позиция = Найти("abc", "b");
    Позиция = СтрНайти("abc", "b");
    Части = СтрРазделить("a,b", ",");
КонецПроцедуры
"#;
        let reported = newer(source, Some("8.3.17"));
        assert!(reported.is_empty(), "{reported:?}");
    }

    #[test]
    fn the_range_covers_the_called_name() {
        let source = r#"
Процедура Тест()
    Результат = СтрЗаменитьПоРегулярномуВыражению("abc", "b", "x");
КонецПроцедуры
"#;
        let diagnostics = run(source, Some("8.3.17"), test_fixture::CfeFixtureBuilder::new(""));
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        let range = diagnostics[0].range;
        let text = &source[usize::from(range.start())..usize::from(range.end())];
        assert_eq!(text, "СтрЗаменитьПоРегулярномуВыражению");
    }

    #[test]
    fn method_of_a_platform_typed_value_is_dated_by_member_and_owner() {
        let source = r#"
Процедура Тест()
    Запрос = Новый HTTPЗапрос("/");
    Запрос.ДобавитьТокенДоступа(Неопределено);
    Запрос.УстановитьТелоИзСтроки("x");
    Информация = Новый СистемнаяИнформация;
    Вариант = Информация.ВариантПриложения;
КонецПроцедуры
"#;
        let reported = newer(source, Some("8.3.17"));
        assert_eq!(reported.len(), 2, "{reported:?}");
        assert!(
            reported.iter().any(|m| m.contains("ДобавитьТокенДоступа") && m.contains("8.3.21")),
            "{reported:?}"
        );
        assert!(
            reported.iter().any(|m| m.contains("ВариантПриложения") && m.contains("8.3.22")),
            "{reported:?}"
        );
    }

    #[test]
    fn constructed_type_newer_than_the_minimum_is_reported() {
        let source = r#"
Процедура Тест()
    Генератор = Новый ГенераторСлучайныхПаролей;
    Список = Новый Массив;
КонецПроцедуры
"#;
        let reported = newer(source, Some("8.3.17"));
        assert_eq!(reported.len(), 1, "{reported:?}");
        assert!(reported[0].contains("ГенераторСлучайныхПаролей"), "{reported:?}");
        assert!(reported[0].contains("8.3.22"), "{reported:?}");
    }

    #[test]
    fn async_declaration_and_await_need_8_3_18() {
        let source = r#"
Асинх Процедура Тест()
    Ждать Пауза();
КонецПроцедуры

Асинх Функция Пауза()
    Возврат Неопределено;
КонецФункции
"#;
        let on_17 = newer(source, Some("8.3.17"));
        assert_eq!(on_17.len(), 3, "two Асинх and one Ждать: {on_17:?}");
        assert_eq!(on_17.iter().filter(|m| m.contains("'Асинх'")).count(), 2, "{on_17:?}");
        assert_eq!(on_17.iter().filter(|m| m.contains("'Ждать'")).count(), 1, "{on_17:?}");
        assert!(on_17.iter().all(|m| m.contains("8.3.18")), "{on_17:?}");

        let on_18 = newer(source, Some("8.3.18"));
        assert!(on_18.is_empty(), "8.3.18 compiles Асинх: {on_18:?}");
    }

    #[test]
    fn unset_minimum_is_silent() {
        let source = r#"
Асинх Процедура Тест()
    Результат = СтрЗаменитьПоРегулярномуВыражению("abc", "b", "x");
    Генератор = Новый ГенераторСлучайныхПаролей;
КонецПроцедуры
"#;
        assert!(newer(source, None).is_empty());
        assert!(newer(source, Some("not a version")).is_empty(), "an unparseable floor is off");
    }

    #[test]
    fn a_local_procedure_of_the_same_name_shadows_the_global() {
        let source = r#"
Функция СтрЗаменитьПоРегулярномуВыражению(Строка, Шаблон, Замена)
    Возврат Строка;
КонецФункции

Процедура Тест()
    Результат = СтрЗаменитьПоРегулярномуВыражению("abc", "b", "x");
КонецПроцедуры
"#;
        let reported = newer(source, Some("8.3.17"));
        assert!(reported.is_empty(), "the call reaches the module's own function: {reported:?}");
    }

    #[test]
    fn a_global_common_module_export_of_the_same_name_shadows_the_global() {
        let mut builder = test_fixture::CfeFixtureBuilder::new("");
        builder.add_base_module_global(
            "Глобальный",
            "Функция СтрЗаменитьПоРегулярномуВыражению(А, Б, В) Экспорт Возврат А; КонецФункции",
        );
        let source = r#"
Процедура Тест()
    Результат = СтрЗаменитьПоРегулярномуВыражению("abc", "b", "x");
КонецПроцедуры
"#;
        let reported: Vec<_> =
            run(source, Some("8.3.17"), builder).into_iter().map(|d| d.message).collect();
        assert!(reported.is_empty(), "{reported:?}");
    }

    /// One undated member exists in the bundled catalog
    /// (`InAppPurchasesManager.ПоддерживаетсяИсторияПриобретений`); its absence of a
    /// date must never read as "new". The lookup layer returns `None` for it, which
    /// is what the inference check needs to stay silent.
    #[test]
    fn a_member_without_a_version_is_silent() {
        let data = bsl_platform::PlatformDataInner::instance();
        let undated = data
            .all_methods()
            .iter()
            .find(|method| method.min_version.is_none())
            .expect("the catalog keeps at least one undated method");
        assert_eq!(
            hir::min_platform::type_member(
                undated.type_name.as_str(),
                undated.name.as_str(),
                false
            ),
            None
        );
    }

    #[test]
    fn an_uncompiled_branch_stays_silent() {
        fn in_server_module(body: &str) -> Vec<String> {
            let mut builder = test_fixture::CfeFixtureBuilder::new("");
            builder.add_base_module("Проба", body);
            crate::test_utils::check_cfe_at_with_db_setup(
                "CommonModules/Проба/Ext/Module.bsl",
                body,
                builder.build(),
                &[],
                DiagnosticsConfig::default(),
                |_| {},
                |db| db.set_min_platform_version(Some("8.3.17".into())),
                |db, ctx| crate::file_diagnostics(db, ctx.file_id, ctx.config),
            )
            .into_iter()
            .filter(|diag| diag.code == DiagnosticCode::PlatformMemberNewerThanMinVersion)
            .map(|diag| diag.message)
            .collect()
        }
        const CALL: &str =
            "    Результат = СтрЗаменитьПоРегулярномуВыражению(\"abc\", \"b\", \"x\");\n";
        let plain = in_server_module(&format!("Процедура Тест() Экспорт\n{CALL}КонецПроцедуры\n"));
        assert_eq!(plain.len(), 1, "the server compiles the plain call: {plain:?}");
        let branch = in_server_module(&format!(
            "Процедура Тест() Экспорт\n#Если ТолстыйКлиентОбычноеПриложение Тогда\n{CALL}#КонецЕсли\nКонецПроцедуры\n"
        ));
        assert!(branch.is_empty(), "a server module never compiles that branch: {branch:?}");
    }

    #[test]
    fn metadata_is_an_active_major_error() {
        let metadata =
            crate::handlers::get_metadata(DiagnosticCode::PlatformMemberNewerThanMinVersion)
                .unwrap();
        assert_eq!(metadata.severity, DiagnosticSeverityLevel::Major);
        assert!(metadata.activated_by_default);
        assert!(!DiagnosticsConfig::default()
            .is_disabled(DiagnosticCode::PlatformMemberNewerThanMinVersion));
    }

    use crate::metadata::DiagnosticSeverityLevel;
}
