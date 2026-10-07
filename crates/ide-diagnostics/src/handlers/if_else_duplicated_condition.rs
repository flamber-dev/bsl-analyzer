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
    activated_by_default: true,
    compatibility_mode: DiagnosticCompatibilityMode::Undefined,
    tags: &[MetadataTag::Suspicious],
    can_locate_on_project: false,
    extra_min_for_complexity: 0.0,
    lsp_severity_override: "",
};

pub fn from_hir(
    first_occurrence_index: usize,
    range: LocalRange,
    ctx: &AnalysisContext,
) -> Option<Diagnostic<LocalRange>> {
    let code = DiagnosticCode::IfElseDuplicatedCondition;

    if ctx.is_disabled_with_metadata(code) {
        return None;
    }

    Some(Diagnostic {
        code,
        message: format!(
            "Дублированное условие в конструкции 'Если...Тогда...ИначеЕсли' (уже использовано в позиции {})",
            first_occurrence_index + 1
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
            .filter(|d| d.code == DiagnosticCode::IfElseDuplicatedCondition)
            .collect();
        expected.assert_eq(&format_diags(code, &diagnostics));
    }

    #[test]
    fn test_no_duplicates() {
        let code = r#"Функция Сигнал(Цвет)
	Если Цвет = "Красный" Тогда
		Возврат "Стоп";
	ИначеЕсли Цвет = "Желтый" Тогда
		Возврат "Внимание";
	ИначеЕсли Цвет = "Зеленый" Тогда
		Возврат "Ехать";
	КонецЕсли;
	Возврат "";
КонецФункции
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_simple_duplicate() {
        let code = r#"Функция Сигнал(Цвет)
	Если Цвет = "Красный" Тогда
		Возврат "Стоп";
	ИначеЕсли Цвет = "Желтый" Тогда
		Возврат "Внимание";
	ИначеЕсли Цвет = "Красный" Тогда
		Возврат "Ехать";
	КонецЕсли;
	Возврат "";
КонецФункции
"#;
        check(
            code,
            expect![[r#"
            IfElseDuplicatedCondition @ 6:12..6:28
              message: Дублированное условие в конструкции 'Если...Тогда...ИначеЕсли' (уже использовано в позиции 1)
              severity: Warning"#]],
        );
    }

    #[test]
    fn test_case_insensitive_variables() {
        let code = r#"Процедура Переключить(Режим)
	Если режим > 2 Тогда
		Режим = 0;
	ИначеЕсли РЕЖИМ > 2 Тогда
		Режим = 1;
	КонецЕсли;
КонецПроцедуры
"#;
        check(
            code,
            expect![[r#"
            IfElseDuplicatedCondition @ 4:12..4:21
              message: Дублированное условие в конструкции 'Если...Тогда...ИначеЕсли' (уже использовано в позиции 1)
              severity: Warning"#]],
        );
    }

    #[test]
    fn test_whitespace_normalization() {
        let code = r#"Процедура Переключить(Режим)
	Если Режим > 2 Тогда
		Режим = 0;
	ИначеЕсли Режим    >  2 Тогда
		Режим = 1;
	КонецЕсли;
КонецПроцедуры
"#;
        check(
            code,
            expect![[r#"
            IfElseDuplicatedCondition @ 4:12..4:25
              message: Дублированное условие в конструкции 'Если...Тогда...ИначеЕсли' (уже использовано в позиции 1)
              severity: Warning"#]],
        );
    }

    #[test]
    fn test_string_case_sensitive() {
        let code = r#"Функция Регистр(Буква)
	Если (Буква = "Я") Тогда
		Возврат "верхний";
	ИначеЕсли (Буква = "я") Тогда
		Возврат "нижний";
	КонецЕсли;
	Возврат "";
КонецФункции
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_string_same_case() {
        let code = r#"Функция Регистр(Буква)
	Если (Буква = "я") Тогда
		Возврат "верхний";
	ИначеЕсли (Буква = "я") Тогда
		Возврат "нижний";
	КонецЕсли;
	Возврат "";
КонецФункции
"#;
        check(
            code,
            expect![[r#"
            IfElseDuplicatedCondition @ 4:12..4:25
              message: Дублированное условие в конструкции 'Если...Тогда...ИначеЕсли' (уже использовано в позиции 1)
              severity: Warning"#]],
        );
    }

    #[test]
    fn test_nested_if_independent() {
        // The inner chain and the outer chain are compared separately.
        let code = r#"Процедура Разобрать(Код)
	Если Код = 10 Тогда
		Если Код = 20 Тогда
			Сообщить("внутри");
		ИначеЕсли Код = 20 Тогда
			Сообщить("повтор внутри");
		КонецЕсли;
	ИначеЕсли Код = 10 Тогда
		Сообщить("повтор снаружи");
	КонецЕсли;
КонецПроцедуры
"#;
        check(
            code,
            expect![[r#"
            IfElseDuplicatedCondition @ 5:13..5:21
              message: Дублированное условие в конструкции 'Если...Тогда...ИначеЕсли' (уже использовано в позиции 1)
              severity: Warning
            IfElseDuplicatedCondition @ 8:12..8:20
              message: Дублированное условие в конструкции 'Если...Тогда...ИначеЕсли' (уже использовано в позиции 1)
              severity: Warning"#]],
        );
    }

    #[test]
    fn test_triple_duplicate_condition() {
        // Every later repeat points at the first occurrence.
        let code = r#"Процедура Разобрать(Код)
	Если Код = 5 Тогда
		Сообщить("пять");
	ИначеЕсли Код = 7 Тогда
		Сообщить("семь");
	ИначеЕсли Код = 9 Тогда
		Сообщить("девять");
	ИначеЕсли Код  =  7 Тогда
		Сообщить("снова семь");
	ИначеЕсли КОД = 7 Тогда
		Сообщить("и ещё");
	Иначе
		Сообщить("прочее");
	КонецЕсли;
КонецПроцедуры
"#;
        check(
            code,
            expect![[r#"
            IfElseDuplicatedCondition @ 8:12..8:21
              message: Дублированное условие в конструкции 'Если...Тогда...ИначеЕсли' (уже использовано в позиции 2)
              severity: Warning
            IfElseDuplicatedCondition @ 10:12..10:19
              message: Дублированное условие в конструкции 'Если...Тогда...ИначеЕсли' (уже использовано в позиции 2)
              severity: Warning"#]],
        );
    }

    #[test]
    fn test_nested_and_outer_duplicates() {
        let code = r#"Процедура Разобрать(Код, Флаг)
	Если Флаг Тогда
		Сообщить("флаг");
	ИначеЕсли Код > 0 Тогда
		Если Код > 100 Тогда
			Сообщить("много");
		ИначеЕсли Код > 10 Тогда
			Сообщить("средне");
		ИначеЕсли Код > 10 Тогда
			Сообщить("снова средне");
		Иначе
			Сообщить("мало");
		КонецЕсли;
	ИначеЕсли Код > 0 Тогда
		Сообщить("снова больше нуля");
	Иначе
		Сообщить("ноль");
	КонецЕсли;
КонецПроцедуры
"#;
        check(
            code,
            expect![[r#"
            IfElseDuplicatedCondition @ 9:13..9:21
              message: Дублированное условие в конструкции 'Если...Тогда...ИначеЕсли' (уже использовано в позиции 2)
              severity: Warning
            IfElseDuplicatedCondition @ 14:12..14:19
              message: Дублированное условие в конструкции 'Если...Тогда...ИначеЕсли' (уже использовано в позиции 2)
              severity: Warning"#]],
        );
    }

    #[test]
    fn test_variable_case_differs_string_case_matters() {
        let code = r#"Функция Регистр(Буква)
	Если (Буква = "я") Тогда
		Возврат 1;
	ИначеЕсли (БУКВА = "я") Тогда
		Возврат 2;
	ИначеЕсли (буква = "Я") Тогда
		Возврат 3;
	Иначе
		Возврат 4;
	КонецЕсли;
КонецФункции
"#;
        check(
            code,
            expect![[r#"
            IfElseDuplicatedCondition @ 4:12..4:25
              message: Дублированное условие в конструкции 'Если...Тогда...ИначеЕсли' (уже использовано в позиции 1)
              severity: Warning"#]],
        );
    }
}
