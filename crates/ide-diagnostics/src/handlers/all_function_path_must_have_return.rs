use crate::define_metadata;
use crate::metadata::*;
use crate::AnalysisContext;
use crate::{Diagnostic, DiagnosticCode};
use hir::LocalRange;
use hir::MethodId;

pub const METADATA: DiagnosticMetadata = define_metadata! {
    diagnostic_type: DiagnosticType::CodeSmell,
    severity: DiagnosticSeverityLevel::Major,
    scope: DiagnosticScope::All,
    modules: &[],
    minutes_to_fix: 1,
    activated_by_default: true,
    compatibility_mode: DiagnosticCompatibilityMode::Undefined,
    tags: &[MetadataTag::Unpredictable, MetadataTag::Badpractice, MetadataTag::Suspicious],
    can_locate_on_project: false,
    extra_min_for_complexity: 0.0,
    lsp_severity_override: "",
};

pub fn from_hir(
    range: LocalRange,
    method_id: &MethodId,
    ctx: &AnalysisContext,
) -> Option<Diagnostic<LocalRange>> {
    let code = DiagnosticCode::AllFunctionPathMustHaveReturn;

    if ctx.is_disabled_with_metadata(code) {
        return None;
    }

    let every_path_returns = ctx
        .method_cfg(*method_id)
        .start()
        .and_then(|start| {
            ctx.method_path_terminates(*method_id).map(|pt| !pt.may_fallthrough_at_block(start))
        })
        .unwrap_or(false);

    if every_path_returns {
        return None;
    }

    Some(Diagnostic {
        code,
        message: message_ru(),
        severity: ctx.severity(code),
        range,
        tags: ctx.tags(code),
        fixes: vec![],
    })
}

fn message_ru() -> String {
    "Не все пути выполнения функции возвращают значение".to_string()
}

#[cfg(test)]
mod tests {
    use crate::test_utils::{check_hir_diagnostic, format_diags};
    use crate::DiagnosticCode;
    use expect_test::expect;

    fn check(code: &str, expected: expect_test::Expect) {
        let diagnostics: Vec<_> = check_hir_diagnostic(code)
            .into_iter()
            .filter(|d| d.code == DiagnosticCode::AllFunctionPathMustHaveReturn)
            .collect();
        expected.assert_eq(&format_diags(code, &diagnostics));
    }

    #[test]
    fn test_missing_return_elseif_no_else() {
        let code = r#"Функция ТарифСтоянки(Знач ТипМашины)
	Если ТипМашины = "Легковая" Тогда
		Возврат 100;
	ИначеЕсли ТипМашины = "Грузовая" Тогда
		Возврат 300;
	ИначеЕсли ТипМашины = "Мотоцикл" Тогда
		Возврат 50;
	КонецЕсли;
КонецФункции
"#;
        check(
            code,
            expect![[r#"
            AllFunctionPathMustHaveReturn @ 1:9..1:21
              message: Не все пути выполнения функции возвращают значение
              severity: Warning"#]],
        );
    }

    #[test]
    fn test_no_diagnostic_explicit_undefined_return() {
        let code = r#"Функция ТарифСтоянки(Знач ТипМашины)
	Если ТипМашины = "Легковая" Тогда
		Возврат 100;
	ИначеЕсли ТипМашины = "Грузовая" Тогда
		Возврат 300;
	КонецЕсли;
	Возврат Неопределено;
КонецФункции
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_missing_return_in_elseif_branch() {
        let code = r#"Функция МестоДляМашины(Знач Машина)
	Если Машина.Электромобиль Тогда
		Возврат "Зарядка";
	ИначеЕсли Машина.Габаритная Тогда
		ОтметитьНегабарит(Машина);
	Иначе
		Возврат "Общее";
	КонецЕсли;
КонецФункции
"#;
        check(
            code,
            expect![[r#"
            AllFunctionPathMustHaveReturn @ 1:9..1:23
              message: Не все пути выполнения функции возвращают значение
              severity: Warning"#]],
        );
    }

    #[test]
    fn test_foreach_loop_no_return_after_loop_emits_diagnostic() {
        // The empty-collection path through Для Каждого reaches the end without Возврат.
        let code = r#"Функция НомерМеста(Места, Номер)
	Для Каждого Место Из Места Цикл
		Если Место.Номер = Номер Тогда
			Возврат Место;
		КонецЕсли;
	КонецЦикла;
КонецФункции
"#;
        check(
            code,
            expect![[r#"
            AllFunctionPathMustHaveReturn @ 1:9..1:19
              message: Не все пути выполнения функции возвращают значение
              severity: Warning"#]],
        );
    }

    #[test]
    fn test_while_true_no_fallback_return_emits_diagnostic() {
        // Without constant propagation `Пока Истина` is treated as a loop that may be skipped.
        let code = r#"Функция ПервоеСвободное(Ярус)
	Пока Истина Цикл
		Если Ярус.Свободно() Тогда
			Возврат Ярус;
		КонецЕсли;
		Ярус = Ярус.Следующий();
	КонецЦикла;
КонецФункции
"#;
        check(
            code,
            expect![[r#"
            AllFunctionPathMustHaveReturn @ 1:9..1:24
              message: Не все пути выполнения функции возвращают значение
              severity: Warning"#]],
        );
    }

    #[test]
    fn test_while_with_break_and_return_after_loop() {
        let code = r#"Функция ЗанятоМест(Камеры)
	Занято = 0;
	Пока Камеры.ЕстьКадр() Цикл
		Если Занято >= 500 Тогда
			Прервать;
		КонецЕсли;
		Занято = Занято + 1
	КонецЦикла;
	Возврат Занято;
КонецФункции
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_simple_missing_else() {
        let code = r#"Функция Скидка(Часы)
	Если Часы > 24 Тогда
		Возврат 10;
	КонецЕсли;
КонецФункции
"#;
        check(
            code,
            expect![[r#"
            AllFunctionPathMustHaveReturn @ 1:9..1:15
              message: Не все пути выполнения функции возвращают значение
              severity: Warning"#]],
        );
    }

    #[test]
    fn test_no_diagnostic_when_all_paths_return() {
        let code = r#"Функция Знак(Баланс)
	Если Баланс > 0 Тогда
		Возврат 1;
	ИначеЕсли Баланс < 0 Тогда
		Возврат -1;
	КонецЕсли;
	Возврат 0;
КонецФункции
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_no_diagnostic_if_else_both_return() {
        let code = r#"Функция АбонементВладельца(Владелец)
	Запрос = Новый Запрос(
	"ВЫБРАТЬ Абонементы.Ссылка КАК Абонемент
	|ИЗ Справочник.Абонементы КАК Абонементы
	|ГДЕ Абонементы.Владелец = &Владелец");
	Запрос.УстановитьПараметр("Владелец", Владелец);
	Выборка = Запрос.Выполнить().Выбрать();
	Если Выборка.Следующий() Тогда
		Возврат Выборка.Абонемент;
	Иначе
		Возврат Справочники.Абонементы.ПустаяСсылка();
	КонецЕсли;
КонецФункции
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_no_diagnostic_preproc_both_branches_return() {
        let code = r#"Функция Источник()
	#Если Сервер Тогда
		Возврат "база";
	#Иначе
		Возврат "кэш";
	#КонецЕсли
КонецФункции
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_missing_return_preproc_else_no_return() {
        let code = r#"Функция Источник()
	#Если Сервер Тогда
		Возврат "база";
	#Иначе
		Источник = "кэш";
	#КонецЕсли
КонецФункции
"#;
        check(
            code,
            expect![[r#"
            AllFunctionPathMustHaveReturn @ 1:9..1:17
              message: Не все пути выполнения функции возвращают значение
              severity: Warning"#]],
        );
    }

    #[test]
    fn test_missing_return_preproc_no_else() {
        let code = r#"Функция Источник()
	#Если Сервер Тогда
		Возврат "база";
	#КонецЕсли
КонецФункции
"#;
        check(
            code,
            expect![[r#"
            AllFunctionPathMustHaveReturn @ 1:9..1:17
              message: Не все пути выполнения функции возвращают значение
              severity: Warning"#]],
        );
    }

    #[test]
    fn test_no_diagnostic_preproc_nested_in_semantic_if() {
        let code = r#"Функция Источник(Онлайн)
	Если Онлайн Тогда
		#Если Сервер Тогда
			Возврат "база";
		#Иначе
			Возврат "кэш";
		#КонецЕсли
	Иначе
		Возврат "файл";
	КонецЕсли
КонецФункции
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_raise_counts_as_exit() {
        let code = r#"Функция ОбязательныйНомер(Номер)
	Если ЗначениеЗаполнено(Номер) Тогда
		Возврат Номер;
	КонецЕсли;
	ВызватьИсключение "Номер не задан";
КонецФункции
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_no_diagnostic_if_no_else_then_try_except_then_return() {
        let code = r#"Функция ОтветШлагбаума(Команда)
	Если Не Команда.Свойство("Код") Тогда
		Возврат "нет кода";
	КонецЕсли;
	Ответ = Новый Структура;
	Попытка
		Ответ.Вставить("Открыт", Шлагбаум.Открыть(Команда.Код));
	Исключение
		Ответ.Вставить("Открыт", Ложь);
	КонецПопытки;
	Возврат Ответ;
КонецФункции
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_no_diagnostic_if_then_try_except_both_return() {
        let code = r#"Функция НомерЯруса(Знач Метка)
	Если Не СтрНачинаетсяС(Метка, "Ярус") Тогда
		Возврат 0;
	КонецЕсли;
	Попытка
		Возврат Число(Сред(Метка, 5));
	Исключение
		Возврат 0;
	КонецПопытки;
КонецФункции
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_no_diagnostic_try_except_both_return() {
        let code = r#"Функция ДоляЗанятых(Занято, Всего)
	Попытка
		Возврат Занято / Всего;
	Исключение
		Возврат 0;
	КонецПопытки;
КонецФункции
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_procedure_not_checked() {
        let code = r#"Процедура ОткрытьШлагбаум(Номер)
	Если Номер = "" Тогда
		Возврат;
	КонецЕсли;
КонецПроцедуры
"#;
        check(code, expect![[r#""#]]);
    }
}
