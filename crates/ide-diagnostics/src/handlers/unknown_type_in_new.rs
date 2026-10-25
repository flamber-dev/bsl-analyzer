use crate::define_metadata;
use crate::metadata::*;
use crate::AnalysisContext;
use crate::{Diagnostic, DiagnosticCode};
use hir::{LocalRange, Name};

pub const METADATA: DiagnosticMetadata = define_metadata! {
    diagnostic_type: DiagnosticType::Error,
    severity: DiagnosticSeverityLevel::Major,
    scope: DiagnosticScope::All,
    modules: &[],
    minutes_to_fix: 5,
    activated_by_default: true,
    compatibility_mode: DiagnosticCompatibilityMode::Undefined,
    tags: &[MetadataTag::Error, MetadataTag::Suspicious],
    can_locate_on_project: false,
    extra_min_for_complexity: 0.0,
    lsp_severity_override: "",
};

/// `Новый <Имя>` with a name the platform does not have: the whole module fails to
/// compile («Тип не определен»).
pub fn from_hir(
    name: &Name,
    range: LocalRange,
    ctx: &AnalysisContext,
) -> Option<Diagnostic<LocalRange>> {
    let message = if matches!(ctx.locale(), base_db::Locale::Ru) {
        format!("Тип '{}' не определен", name.as_str())
    } else {
        format!("Type '{}' is not defined", name.as_str())
    };
    crate::simple_hir_diagnostic(DiagnosticCode::UnknownTypeInNew, message, range, ctx)
}

#[cfg(test)]
mod tests {
    use crate::test_utils::check_hir_diagnostic_with_fixtures;

    const CODE: &str = "UnknownTypeInNew";

    fn findings(body: &str) -> Vec<String> {
        let fixture = format!("//- /test.bsl\nПроцедура Проба()\n{body}\nКонецПроцедуры\n");
        check_hir_diagnostic_with_fixtures(&fixture)
            .into_iter()
            .filter(|d| d.code.as_str() == CODE)
            .map(|d| d.message)
            .collect()
    }

    #[test]
    fn unknown_type_is_reported_with_its_name() {
        let found = findings("    Т = Новый НесуществующийТипОбъекта;");
        assert_eq!(found.len(), 1, "got: {found:?}");
        assert!(found[0].contains("НесуществующийТипОбъекта"), "got: {found:?}");
    }

    #[test]
    fn unknown_type_with_arguments_is_reported() {
        let found = findings("    Т = Новый НесуществующийТип(1, 2);");
        assert_eq!(found.len(), 1, "got: {found:?}");
    }

    #[test]
    fn unknown_english_type_is_reported() {
        let found = findings("    Т = New NoSuchTypeAtAll;");
        assert_eq!(found.len(), 1, "got: {found:?}");
    }

    #[test]
    fn known_types_are_silent() {
        for ctor in [
            "Новый Массив",
            "Новый Array",
            "новый массив",
            "НОВЫЙ МАССИВ",
            "Новый Структура(\"А\", 1)",
            "Новый ТаблицаЗначений",
            "Новый COMОбъект(\"Scripting.FileSystemObject\")",
            "Новый Запрос",
        ] {
            let found = findings(&format!("    Т = {ctor};"));
            assert!(found.is_empty(), "`{ctor}` is a platform type, got: {found:?}");
        }
    }

    #[test]
    fn legacy_platform_type_name_is_silent() {
        let found = findings("    Блокировка = Новый БлокировкаУстановкиСоединений;");
        assert!(found.is_empty(), "the platform still compiles the old name, got: {found:?}");
    }

    #[test]
    fn name_given_by_string_is_not_judged() {
        for ctor in ["Новый(\"Массив\")", "Новый(\"НетТакого\")", "Новый(ИмяТипа)"]
        {
            let found = findings(&format!("    ИмяТипа = \"Массив\";\n    Т = {ctor};"));
            assert!(found.is_empty(), "`{ctor}` names the type by value, got: {found:?}");
        }
    }

    #[test]
    fn type_available_in_another_environment_is_not_unknown() {
        let fixture = "//- /Catalogs/Товары/Forms/ФормаЭлемента/Ext/Form/Module.bsl\n\
                       &НаКлиенте\nПроцедура Проба()\n    Т = Новый ЧтениеТекста;\nКонецПроцедуры\n";
        let found: Vec<_> = check_hir_diagnostic_with_fixtures(fixture)
            .into_iter()
            .filter(|d| d.code.as_str() == CODE)
            .collect();
        assert!(found.is_empty(), "a platform type, however unavailable: {found:?}");
    }
}
