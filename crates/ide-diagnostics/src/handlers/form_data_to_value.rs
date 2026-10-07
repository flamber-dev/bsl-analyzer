use crate::define_metadata;
use crate::metadata::*;
use crate::AnalysisContext;
use crate::{Diagnostic, DiagnosticCode};
use hir::LocalRange;

pub const METADATA: DiagnosticMetadata = define_metadata! {
    diagnostic_type: DiagnosticType::CodeSmell,
    severity: DiagnosticSeverityLevel::Info,
    scope: DiagnosticScope::Bsl,
    modules: &[],
    minutes_to_fix: 5,
    activated_by_default: true,
    compatibility_mode: DiagnosticCompatibilityMode::Undefined,
    tags: &[MetadataTag::Badpractice],
    can_locate_on_project: false,
    extra_min_for_complexity: 0.0,
    lsp_severity_override: "",
};

pub fn from_hir(range: LocalRange, ctx: &AnalysisContext) -> Option<Diagnostic<LocalRange>> {
    crate::simple_hir_diagnostic(
        DiagnosticCode::FormDataToValue,
        "Обнаружено использование метода ДанныеФормыВЗначение",
        range,
        ctx,
    )
}

#[cfg(test)]
mod tests {
    use crate::test_utils::*;
    use crate::DiagnosticCode;
    use expect_test::expect;

    fn check(code: &str, expected: expect_test::Expect) {
        let diagnostics: Vec<_> = check_hir_diagnostic(code)
            .into_iter()
            .filter(|d| d.code == DiagnosticCode::FormDataToValue)
            .collect();
        expected.assert_eq(&format_diags(code, &diagnostics));
    }

    #[test]
    fn test_form_context_methods_are_reported() {
        let code = r#"&НаСервере
Процедура ЗагрузитьСписок()
	Таблица = ДанныеФормыВЗначение(Список, Тип("ТаблицаЗначений"));
КонецПроцедуры

&НаКлиенте
Процедура ПоказатьСписок()
	ДанныеФормыВЗначение(Список, Тип("ТаблицаЗначений"));
КонецПроцедуры

Функция КопияСписка()
	Возврат ДанныеФормыВЗначение(Список, Тип("ТаблицаЗначений"));
КонецФункции
"#;
        check(
            code,
            expect![[r#"
            FormDataToValue @ 3:12..3:32
              message: Обнаружено использование метода ДанныеФормыВЗначение
              severity: Hint
            FormDataToValue @ 8:2..8:22
              message: Обнаружено использование метода ДанныеФормыВЗначение
              severity: Hint
            FormDataToValue @ 12:10..12:30
              message: Обнаружено использование метода ДанныеФормыВЗначение
              severity: Hint"#]],
        );
    }

    #[test]
    fn test_contextless_methods_are_silent() {
        let code = r#"&НаСервереБезКонтекста
Процедура ЗагрузитьСписок()
	Таблица = ДанныеФормыВЗначение(Список, Тип("ТаблицаЗначений"));
КонецПроцедуры

&НаКлиентеНаСервереБезКонтекста
Функция КопияСписка()
	Возврат ДанныеФормыВЗначение(Список, Тип("ТаблицаЗначений"));
КонецФункции
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_qualified_calls_are_silent() {
        // No platform type declares this method: with a receiver it is a different method
        // that merely shares the spelling.
        let code = r#"&НаСервере
Процедура СвернутьОстатки()
	ОкноОстатков = ПолучитьФорму("Обработка.Остатки.Форма");
	Остатки = ОкноОстатков.ДанныеФормыВЗначение(Отбор, Тип("ДеревоЗначений"));
	ЭтотОбъект.ДанныеФормыВЗначение(Отбор, Тип("ДеревоЗначений"));
КонецПроцедуры
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_english_qualified_and_bare_calls() {
        let code = r#"&AtServer
Procedure FoldBalances()
	Balances = BalanceForm.FormDataToValue(Filter, Type("ValueTree"));
	Tree = FormDataToValue(Filter, Type("ValueTree"));
EndProcedure
"#;
        check(
            code,
            expect![[r#"
            FormDataToValue @ 4:9..4:24
              message: Обнаружено использование метода ДанныеФормыВЗначение
              severity: Hint"#]],
        );
    }

    #[test]
    fn test_case_insensitive() {
        let code = r#"Процедура СверитьКопии()
	Первая = данныеформывзначение(Список, Тип("Массив"));
	Вторая = ДАННЫЕФОРМЫвЗНАЧЕНИЕ(Список, Тип("Массив"));
КонецПроцедуры
"#;
        check(
            code,
            expect![[r#"
            FormDataToValue @ 2:11..2:31
              message: Обнаружено использование метода ДанныеФормыВЗначение
              severity: Hint
            FormDataToValue @ 3:11..3:31
              message: Обнаружено использование метода ДанныеФормыВЗначение
              severity: Hint"#]],
        );
    }

    #[test]
    fn test_no_call_ignored() {
        let code = r#"Процедура ЗапомнитьОбработчик()
	Обработчик = ДанныеФормыВЗначение;
КонецПроцедуры
"#;
        check(code, expect![[r#""#]]);
    }
}
