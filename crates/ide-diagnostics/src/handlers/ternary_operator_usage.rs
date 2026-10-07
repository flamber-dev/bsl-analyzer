use crate::define_metadata;
use crate::metadata::*;
use crate::AnalysisContext;
use crate::{Diagnostic, DiagnosticCode};
use hir::LocalRange;

pub const METADATA: DiagnosticMetadata = define_metadata! {
    diagnostic_type: DiagnosticType::CodeSmell,
    severity: DiagnosticSeverityLevel::Minor,
    scope: DiagnosticScope::All,
    modules: &[],
    minutes_to_fix: 3,
    activated_by_default: false,
    compatibility_mode: DiagnosticCompatibilityMode::Undefined,
    tags: &[MetadataTag::Brainoverload],
    can_locate_on_project: false,
    extra_min_for_complexity: 0.0,
    lsp_severity_override: "",
};

pub fn from_hir(range: LocalRange, ctx: &AnalysisContext) -> Option<Diagnostic<LocalRange>> {
    crate::simple_hir_diagnostic(
        DiagnosticCode::TernaryOperatorUsage,
        "Используйте конструкцию Если-Иначе вместо тернарного оператора",
        range,
        ctx,
    )
}

#[cfg(test)]
mod tests {
    use crate::test_utils::*;
    use crate::{DiagnosticCode, DiagnosticsConfig};
    use expect_test::expect;

    fn check(code: &str, expected: expect_test::Expect) {
        check_diagnostics_snapshot_for(code, DiagnosticCode::TernaryOperatorUsage, expected);
    }

    #[test]
    fn test_if_else_is_silent() {
        let code = r#"Функция ТарифДоставки(Заказ)
	Если Заказ.Срочный Тогда
		Ставка = 2;
	Иначе
		Ставка = 1;
	КонецЕсли;
	Возврат Ставка;
КонецФункции
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_simple_ternary() {
        let code = r#"Функция ТарифДоставки(Заказ)
	Ставка = ?(Заказ.Срочный, 2, 1);
	Возврат Ставка;
КонецФункции
"#;
        check(
            code,
            expect![[r#"
            TernaryOperatorUsage @ 2:11..2:33
              message: Используйте конструкцию Если-Иначе вместо тернарного оператора
              severity: Information"#]],
        );
    }

    #[test]
    fn test_ternary_in_condition() {
        let code = r#"Процедура ПроверитьУпаковку(Заказ)
	Если ?(Заказ.Вес > 30, Истина, Заказ.Хрупкий) Тогда
		Заказ.НужнаОбрешетка = Истина;
	КонецЕсли;
КонецПроцедуры
"#;
        check(
            code,
            expect![[r#"
            TernaryOperatorUsage @ 2:7..2:47
              message: Используйте конструкцию Если-Иначе вместо тернарного оператора
              severity: Information"#]],
        );
    }

    #[test]
    fn test_nested_ternary() {
        let code = r#"Ставка = ?(Заказ.Срочный
	, ?(Заказ.Вес > 30
		, 5
		, 3)
	, 1);
"#;
        check(
            code,
            expect![[r#"
            TernaryOperatorUsage @ 1:10..5:6
              message: Используйте конструкцию Если-Иначе вместо тернарного оператора
              severity: Information
            TernaryOperatorUsage @ 2:4..4:7
              message: Используйте конструкцию Если-Иначе вместо тернарного оператора
              severity: Information"#]],
        );
    }

    #[test]
    fn test_disabled_by_default() {
        let code = r#"Функция ТарифДоставки(Заказ)
	Возврат ?(Заказ.Срочный, 2, 1);
КонецФункции
"#;
        let diagnostics =
            check_hir_diagnostic_with_config(code, DiagnosticsConfig::default(), |ctx| {
                crate::diagnostics(ctx)
            });
        let diags: Vec<_> =
            diagnostics.iter().filter(|d| d.code == DiagnosticCode::TernaryOperatorUsage).collect();

        assert_eq!(diags.len(), 0, "Should be disabled by default");
    }
}
