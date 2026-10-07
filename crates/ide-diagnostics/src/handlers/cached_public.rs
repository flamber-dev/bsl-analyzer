use crate::define_metadata;
use crate::metadata::*;
use crate::{Diagnostic, DiagnosticCode, DiagnosticsContext};
use bsl_metadata::ReturnValueReuse;
use stdx::case::CaseExt;

pub const METADATA: DiagnosticMetadata = define_metadata! {
    diagnostic_type: DiagnosticType::CodeSmell,
    severity: DiagnosticSeverityLevel::Major,
    scope: DiagnosticScope::Bsl,
    modules: &[bsl_metadata::ModuleType::CommonModule],
    minutes_to_fix: 5,
    activated_by_default: true,
    compatibility_mode: DiagnosticCompatibilityMode::Undefined,
    tags: &[MetadataTag::Standard, MetadataTag::Design],
    can_locate_on_project: false,
    extra_min_for_complexity: 0.0,
    lsp_severity_override: "",
    clean_code_attribute: CleanCodeAttribute::Adaptable,
};

pub fn check(ctx: &DiagnosticsContext) -> Vec<Diagnostic> {
    let code = DiagnosticCode::CachedPublic;
    if ctx.is_disabled_with_metadata(code) {
        return Vec::new();
    }

    let region_tree = ctx.region_tree();

    let public_regions: Vec<_> = region_tree
        .regions()
        .filter(|(_, region)| is_public_region(region.name.as_str()))
        .collect();

    if public_regions.is_empty() {
        return Vec::new();
    }

    let metadata = ctx.module_metadata();

    let common_module = match &metadata.common_module {
        Some(cm) => cm,
        None => return Vec::new(),
    };

    if !is_cached_reuse(common_module.return_values_reuse()) {
        return Vec::new();
    }

    let item_tree = ctx.item_tree();

    public_regions
        .into_iter()
        .filter_map(|(_, region)| {
            let has_methods = item_tree
                .procedures()
                .any(|(_, proc)| region.range.contains_range(proc.source_range))
                || item_tree
                    .functions()
                    .any(|(_, func)| region.range.contains_range(func.source_range));

            if has_methods {
                Some(Diagnostic {
                    code: DiagnosticCode::CachedPublic,
                    message: "Кэшируемый модуль не должен содержать методы в публичных областях"
                        .to_string(),
                    severity: ctx.severity(code),
                    range: region.range,
                    tags: ctx.tags(code),
                    fixes: vec![],
                })
            } else {
                None
            }
        })
        .collect()
}

fn is_cached_reuse(reuse: ReturnValueReuse) -> bool {
    matches!(reuse, ReturnValueReuse::DuringRequest | ReturnValueReuse::DuringSession)
}

fn is_public_region(region_name: &str) -> bool {
    let name_lower = region_name.fold_lower();
    name_lower == "public" || name_lower == "программныйинтерфейс"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{
        check_ast_diagnostic, check_metadata_diagnostic, format_diags, make_common_module_metadata,
    };
    use expect_test::expect;

    /// Diagnostics of `code` as a common module with the given reuse mode.
    fn check_cached(reuse: ReturnValueReuse, code: &str) -> Vec<Diagnostic> {
        let module = bsl_metadata::CommonModule::builder()
            .name("СкладскиеНастройкиПовтИсп")
            .return_values_reuse(reuse)
            .build();
        check_metadata_diagnostic(make_common_module_metadata(module), code, |_, ctx| check(ctx))
    }

    const MODULE: &str = r#"#Область ПрограммныйИнтерфейс

Функция ЕдиницаХранения(Номенклатура) Экспорт
	Возврат Номенклатура.ЕдиницаХранения;
КонецФункции

#КонецОбласти

#Область СлужебныеПроцедурыИФункции

Функция ПрочитатьНастройки()
	Возврат Новый Структура;
КонецФункции

#КонецОбласти

#Область Public

Function DefaultWarehouse() Export
	Return Undefined;
EndFunction

#EndRegion
"#;

    #[test]
    fn test_during_request_finds_public_regions() {
        let diagnostics = check_cached(ReturnValueReuse::DuringRequest, MODULE);
        expect![[r#"
            CachedPublic @ 1:1..7:14
              message: Кэшируемый модуль не должен содержать методы в публичных областях
              severity: Warning
            CachedPublic @ 17:1..23:11
              message: Кэшируемый модуль не должен содержать методы в публичных областях
              severity: Warning"#]]
        .assert_eq(&format_diags(MODULE, &diagnostics));
    }

    #[test]
    fn test_during_session_finds_public_regions() {
        let diagnostics = check_cached(ReturnValueReuse::DuringSession, MODULE);
        expect![[r#"
            CachedPublic @ 1:1..7:14
              message: Кэшируемый модуль не должен содержать методы в публичных областях
              severity: Warning
            CachedPublic @ 17:1..23:11
              message: Кэшируемый модуль не должен содержать методы в публичных областях
              severity: Warning"#]]
        .assert_eq(&format_diags(MODULE, &diagnostics));
    }

    #[test]
    fn test_dont_use_skips_check() {
        let diagnostics = check_cached(ReturnValueReuse::DontUse, MODULE);
        assert_eq!(diagnostics.len(), 0, "DontUse means not cached");
    }

    #[test]
    fn test_no_common_module_metadata() {
        let diagnostics = check_ast_diagnostic(MODULE, check);
        assert_eq!(diagnostics.len(), 0, "Should skip when no CommonModule metadata");
    }

    #[test]
    fn test_non_public_region_ignored() {
        let code = r#"#Область СлужебныйПрограммныйИнтерфейс
Функция КлючКэша(Склад) Экспорт
	Возврат Склад.Код;
КонецФункции
#КонецОбласти
"#;
        let diagnostics = check_cached(ReturnValueReuse::DuringRequest, code);
        assert_eq!(diagnostics.len(), 0);
    }

    #[test]
    fn test_empty_public_region() {
        let code = r#"#Область ПрограммныйИнтерфейс
// Методы перенесены в обычный общий модуль.
#КонецОбласти

#Область СлужебныеПроцедурыИФункции
Функция КлючКэша(Склад)
	Возврат Склад.Код;
КонецФункции
#КонецОбласти
"#;
        let diagnostics = check_cached(ReturnValueReuse::DuringRequest, code);
        assert_eq!(diagnostics.len(), 0);
    }

    #[test]
    fn test_multiple_methods_in_public_region() {
        let code = r#"#Область программныйинтерфейс
Процедура СброситьКэш() Экспорт
КонецПроцедуры
Функция КлючКэша(Склад) Экспорт
	Возврат Склад.Код;
КонецФункции
#КонецОбласти
"#;
        let diagnostics = check_cached(ReturnValueReuse::DuringSession, code);
        expect![[r#"
            CachedPublic @ 1:1..7:14
              message: Кэшируемый модуль не должен содержать методы в публичных областях
              severity: Warning"#]]
        .assert_eq(&format_diags(code, &diagnostics));
    }

    #[test]
    fn test_is_cached_reuse() {
        assert!(is_cached_reuse(ReturnValueReuse::DuringSession));
        assert!(is_cached_reuse(ReturnValueReuse::DuringRequest));
        assert!(!is_cached_reuse(ReturnValueReuse::DontUse));
        assert!(!is_cached_reuse(ReturnValueReuse::Unknown));
    }

    #[test]
    fn test_is_public_region_russian() {
        assert!(is_public_region("программныйИнтерфейс"));
        assert!(is_public_region("ПРОГРАММНЫЙинтерфейс"));
        assert!(is_public_region("ПрограммныйИнтерфейс"));
    }

    #[test]
    fn test_is_public_region_english() {
        assert!(is_public_region("pUBLIC"));
        assert!(is_public_region("Public"));
    }

    #[test]
    fn test_is_not_public_region() {
        assert!(!is_public_region("СлужебныйПрограммныйИнтерфейс"));
        assert!(!is_public_region("ПрограммныйИнтерфейсСклада"));
        assert!(!is_public_region("Internal"));
        assert!(!is_public_region(""));
    }
}
