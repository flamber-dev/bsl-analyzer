use crate::define_metadata;
use crate::metadata::*;
use crate::AnalysisContext;
use crate::{Diagnostic, DiagnosticCode};
use hir::LocalRange;

pub const METADATA: DiagnosticMetadata = define_metadata! {
    diagnostic_type: DiagnosticType::CodeSmell,
    severity: DiagnosticSeverityLevel::Major,
    scope: DiagnosticScope::All,
    modules: &[],
    minutes_to_fix: 10,
    activated_by_default: false,
    compatibility_mode: DiagnosticCompatibilityMode::Undefined,
    tags: &[MetadataTag::Design],
    can_locate_on_project: false,
    extra_min_for_complexity: 0.0,
    lsp_severity_override: "",
};

pub fn from_hir(
    name: &str,
    range: LocalRange,
    ctx: &AnalysisContext,
) -> Option<Diagnostic<LocalRange>> {
    let code = DiagnosticCode::FunctionOutParameter;

    if ctx.is_disabled_with_metadata(code) {
        return None;
    }

    Some(Diagnostic {
        code,
        message: format!(
            "Функция изменяет параметр '{}'. Используйте возвращаемое значение вместо выходного параметра",
            name
        ),
        severity: ctx.severity(code),
        range,
        tags: ctx.tags(code),
        fixes: vec![],
    })
}

#[cfg(test)]
mod tests {
    use crate::test_utils::*;
    use crate::DiagnosticCode;
    use expect_test::expect;

    fn check(code: &str, expected: expect_test::Expect) {
        let diagnostics: Vec<_> = check_hir_diagnostic(code)
            .into_iter()
            .filter(|d| d.code == DiagnosticCode::FunctionOutParameter)
            .collect();
        expected.assert_eq(&format_diags(code, &diagnostics));
    }

    #[test]
    fn test_local_and_val_parameters_are_silent() {
        let code = r#"Функция СтоимостьДоставки(Маршрут, Знач Тариф)
	Расстояние = Маршрут.Длина;
	Тариф = Тариф * 2;
	Маршрут.Проверен = Истина;
	Если Маршрут = Неопределено Тогда
		Возврат 0;
	КонецЕсли;
	Возврат Расстояние * Тариф;
КонецФункции
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_function_out_parameter() {
        let code = r#"Функция СтоимостьДоставки(Маршрут, Знач Тариф)
	маршрут = Маршрут.Длина;
	Тариф = Тариф * 2;
	Возврат Тариф;
КонецФункции
"#;
        check(
            code,
            expect![[r#"
            FunctionOutParameter @ 2:2..2:9
              message: Функция изменяет параметр 'маршрут'. Используйте возвращаемое значение вместо выходного параметра
              severity: Warning"#]],
        );
    }

    #[test]
    fn test_case_insensitive() {
        let code = r#"Функция НормализоватьКод(КодТовара)
	КОДТОВАРА = СокрЛП(КодТовара);
	Возврат кодтовара;
КонецФункции
"#;
        check(
            code,
            expect![[r#"
            FunctionOutParameter @ 2:2..2:11
              message: Функция изменяет параметр 'КОДТОВАРА'. Используйте возвращаемое значение вместо выходного параметра
              severity: Warning"#]],
        );
    }

    #[test]
    fn test_procedure_allowed() {
        let code = r#"Процедура ЗаполнитьМаршрут(Маршрут, Знач Склад)
	Маршрут = Новый Структура("Склад", Склад);
КонецПроцедуры
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_multiple_violations() {
        let code = r#"Функция РазобратьАдрес(Адрес, Индекс, Знач Страна)
	Индекс = Лев(Адрес, 6);
	Адрес = Сред(Адрес, 8);
	Возврат Страна <> "";
КонецФункции
"#;
        check(
            code,
            expect![[r#"
            FunctionOutParameter @ 2:2..2:8
              message: Функция изменяет параметр 'Индекс'. Используйте возвращаемое значение вместо выходного параметра
              severity: Warning
            FunctionOutParameter @ 3:2..3:7
              message: Функция изменяет параметр 'Адрес'. Используйте возвращаемое значение вместо выходного параметра
              severity: Warning"#]],
        );
    }
}
