use crate::define_metadata;
use crate::metadata::*;
use crate::AnalysisContext;
use crate::{Diagnostic, DiagnosticCode};
use hir::LocalRange;

pub const METADATA: DiagnosticMetadata = define_metadata! {
    diagnostic_type: DiagnosticType::Error,
    severity: DiagnosticSeverityLevel::Major,
    scope: DiagnosticScope::All,
    modules: &[],
    minutes_to_fix: 10,
    activated_by_default: true,
    compatibility_mode: DiagnosticCompatibilityMode::Undefined,
    tags: &[MetadataTag::Suspicious, MetadataTag::Unpredictable],
    can_locate_on_project: false,
    extra_min_for_complexity: 0.0,
    lsp_severity_override: "",
};

pub fn from_hir(range: LocalRange, ctx: &AnalysisContext) -> Option<Diagnostic<LocalRange>> {
    crate::simple_hir_diagnostic(
        DiagnosticCode::FunctionShouldHaveReturn,
        "Функция должна содержать хотя бы один оператор Возврат",
        range,
        ctx,
    )
}

#[cfg(test)]
mod tests {
    use crate::test_utils::{check_hir_diagnostic, format_diags};
    use crate::DiagnosticCode;
    use expect_test::expect;

    fn check(code: &str, expected: expect_test::Expect) {
        let diagnostics: Vec<_> = check_hir_diagnostic(code)
            .into_iter()
            .filter(|d| d.code == DiagnosticCode::FunctionShouldHaveReturn)
            .collect();
        expected.assert_eq(&format_diags(code, &diagnostics));
    }

    #[test]
    fn test_function_with_return() {
        let code = r#"Функция ОстатокЛимита(Лимит, Израсходовано)
	Остаток = Лимит - Израсходовано;
	Возврат Макс(Остаток, 0);
КонецФункции
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_function_without_return() {
        let code = r#"Функция ОстатокЛимита(Лимит, Израсходовано)
	Остаток = Лимит - Израсходовано;
	Остаток = Макс(Остаток, 0);
КонецФункции
"#;
        check(
            code,
            expect![[r#"
            FunctionShouldHaveReturn @ 1:9..1:22
              message: Функция должна содержать хотя бы один оператор Возврат
              severity: Major"#]],
        );
    }

    #[test]
    fn test_return_on_one_path_is_enough() {
        // Presence of a single Возврат satisfies this rule; whether every path returns is
        // AllFunctionPathMustHaveReturn's question, not this one's.
        let code = r#"Функция ПервыйСвободныйСлот(Слоты)
	Для Каждого Слот Из Слоты Цикл
		Если Слот.Свободен Тогда
			Возврат Слот;
		КонецЕсли;
	КонецЦикла;
КонецФункции
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_procedures_need_no_return() {
        let code = r#"Процедура ОчиститьЖурнал(Журнал)
	Журнал.Очистить();
КонецПроцедуры

Процедура Заглушка()
КонецПроцедуры

Процедура ПрерватьОбход(Флаг)
	Если Флаг Тогда
		Возврат;
	КонецЕсли;
КонецПроцедуры
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_only_the_function_without_return_in_a_module() {
        let code = r#"Процедура Подготовить()
	Счетчик = 0;
КонецПроцедуры

Функция КодСклада(Склад)
	Возврат Склад.Код;
КонецФункции

Функция ПутьКАрхиву(Каталог)
	Путь = Каталог + "/архив";
КонецФункции

Функция ЕстьОшибки(Протокол)
	Возврат Протокол.Количество() > 0;
КонецФункции
"#;
        check(
            code,
            expect![[r#"
            FunctionShouldHaveReturn @ 9:9..9:20
              message: Функция должна содержать хотя бы один оператор Возврат
              severity: Major"#]],
        );
    }

    #[test]
    fn test_english_function_with_return() {
        let code = r#"Function Clamp(Value, Upper)
	If Value > Upper Then
		Return Upper;
	EndIf;
	Return Value;
EndFunction
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_english_function_without_return() {
        let code = r#"Function Clamp(Value, Upper)
	If Value > Upper Then
		Value = Upper;
	EndIf;
EndFunction
"#;
        check(
            code,
            expect![[r#"
            FunctionShouldHaveReturn @ 1:10..1:15
              message: Функция должна содержать хотя бы один оператор Возврат
              severity: Major"#]],
        );
    }
}
