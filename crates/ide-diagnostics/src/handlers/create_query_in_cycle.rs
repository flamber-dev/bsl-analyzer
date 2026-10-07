use crate::define_metadata;
use crate::metadata::*;
use crate::AnalysisContext;
use crate::{Diagnostic, DiagnosticCode};
use hir::LocalRange;

pub const METADATA: DiagnosticMetadata = define_metadata! {
    diagnostic_type: DiagnosticType::Error,
    severity: DiagnosticSeverityLevel::Critical,
    scope: DiagnosticScope::All,
    modules: &[],
    minutes_to_fix: 20,
    activated_by_default: true,
    compatibility_mode: DiagnosticCompatibilityMode::Undefined,
    tags: &[MetadataTag::Performance],
    can_locate_on_project: false,
    extra_min_for_complexity: 0.0,
    lsp_severity_override: "",
};

pub fn from_hir(range: LocalRange, ctx: &AnalysisContext) -> Option<Diagnostic<LocalRange>> {
    crate::simple_hir_diagnostic(
        DiagnosticCode::CreateQueryInCycle,
        "Выполнение запроса в цикле приводит к деградации производительности. \
         Создайте запрос один раз до цикла и изменяйте только параметры внутри цикла",
        range,
        ctx,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::check_diagnostics_snapshot_for;
    use expect_test::expect;

    fn check(code: &str, expected: expect_test::Expect) {
        check_diagnostics_snapshot_for(code, DiagnosticCode::CreateQueryInCycle, expected);
    }

    #[test]
    fn test_query_executed_outside_loop() {
        let code = r#"Функция ЦеныТарифов(Тарифы)
	Запрос = Новый Запрос("ВЫБРАТЬ Т.Цена ИЗ Справочник.Тарифы КАК Т ГДЕ Т.Ссылка В (&Тарифы)");
	Запрос.УстановитьПараметр("Тарифы", Тарифы);
	Выборка = Запрос.Выполнить().Выбрать();
	Цены = Новый Массив;
	Пока Выборка.Следующий() Цикл
		Цены.Добавить(Выборка.Цена);
	КонецЦикла;
	Возврат Цены;
КонецФункции
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_query_created_outside_loop_but_executed_inside_loop() {
        let code = r#"Функция ЦеныТарифов(Тарифы)
	Запрос = Новый Запрос("ВЫБРАТЬ Т.Цена ИЗ Справочник.Тарифы КАК Т ГДЕ Т.Ссылка = &Тариф");
	Цены = Новый Массив;
	Для Каждого Тариф Из Тарифы Цикл
		Запрос.УстановитьПараметр("Тариф", Тариф);
		Цены.Добавить(Запрос.Выполнить().Выгрузить());
	КонецЦикла;
	Возврат Цены;
КонецФункции
"#;
        check(
            code,
            expect![[r#"
            CreateQueryInCycle @ 6:17..6:35
              message: Выполнение запроса в цикле приводит к деградации производительности. Создайте запрос один раз до цикла и изменяйте только параметры внутри цикла
              severity: Critical"#]],
        );
    }

    #[test]
    fn test_query_created_and_executed_in_while_loop() {
        let code = r#"Процедура ОбойтиСтраницы(Курсор)
	Пока Курсор.ЕстьЕще() Цикл
		Страница = Новый Запрос;
		Страница.Текст = Курсор.ТекстСтраницы();
		Страница.Выполнить();
	КонецЦикла;
КонецПроцедуры
"#;
        check(
            code,
            expect![[r#"
            CreateQueryInCycle @ 5:3..5:23
              message: Выполнение запроса в цикле приводит к деградации производительности. Создайте запрос один раз до цикла и изменяйте только параметры внутри цикла
              severity: Critical"#]],
        );
    }

    #[test]
    fn test_english_keywords() {
        let code = r#"Procedure WalkPages(Pages)
	For Each Page In Pages Do
		Request = New Query(Page.Text);
		Request.Execute();
	EndDo;
EndProcedure
"#;
        check(
            code,
            expect![[r#"
            CreateQueryInCycle @ 4:3..4:20
              message: Выполнение запроса в цикле приводит к деградации производительности. Создайте запрос один раз до цикла и изменяйте только параметры внутри цикла
              severity: Critical"#]],
        );
    }

    #[test]
    fn test_case_insensitive() {
        let code = r#"Процедура ОбойтиСтраницы(ЧислоСтраниц)
	Для Номер = 1 По ЧислоСтраниц Цикл
		Страница = Новый запрос;
		Страница.выполнить();
	КонецЦикла;
КонецПроцедуры
"#;
        check(
            code,
            expect![[r#"
            CreateQueryInCycle @ 4:3..4:23
              message: Выполнение запроса в цикле приводит к деградации производительности. Создайте запрос один раз до цикла и изменяйте только параметры внутри цикла
              severity: Critical"#]],
        );
    }

    #[test]
    fn test_query_builder() {
        let code = r#"Процедура ПостроитьОтчеты(Периоды)
	Построитель = Новый ПостроительЗапроса;
	Для Каждого Период Из Периоды Цикл
		Построитель.Выполнить();
	КонецЦикла;
КонецПроцедуры
"#;
        check(
            code,
            expect![[r#"
            CreateQueryInCycle @ 4:3..4:26
              message: Выполнение запроса в цикле приводит к деградации производительности. Создайте запрос один раз до цикла и изменяйте только параметры внутри цикла
              severity: Critical"#]],
        );
    }
}
