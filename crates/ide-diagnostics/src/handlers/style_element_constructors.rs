use crate::define_metadata;
use crate::metadata::*;
use crate::AnalysisContext;
use crate::{Diagnostic, DiagnosticCode};
use hir::LocalRange;

pub const METADATA: DiagnosticMetadata = define_metadata! {
    diagnostic_type: DiagnosticType::Error,
    severity: DiagnosticSeverityLevel::Minor,
    scope: DiagnosticScope::Bsl,
    modules: &[],
    minutes_to_fix: 5,
    activated_by_default: true,
    compatibility_mode: DiagnosticCompatibilityMode::Undefined,
    tags: &[MetadataTag::Standard, MetadataTag::Badpractice],
    can_locate_on_project: false,
    extra_min_for_complexity: 0.0,
    lsp_severity_override: "",
};

pub fn from_hir(
    type_name: &str,
    range: LocalRange,
    ctx: &AnalysisContext,
) -> Option<Diagnostic<LocalRange>> {
    let code = DiagnosticCode::StyleElementConstructors;

    if ctx.is_disabled_with_metadata(code) {
        return None;
    }

    Some(Diagnostic {
        code,
        message: format!("Замените конструктор {} на получение элемента стиля", type_name),
        severity: ctx.severity(code),
        range,
        tags: ctx.tags(code),
        fixes: vec![],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::check_diagnostics_snapshot_for;
    use expect_test::expect;

    #[test]
    fn test_style_items_and_other_constructors_are_silent() {
        let code = r#"Процедура ОформитьПоле(Поле)
	Поле.ЦветТекста = ЦветаСтиля.ЦветОсобогоТекста;
	Поле.Шрифт = ШрифтыСтиля.ШрифтЗаголовка;
	Параметры = Новый Структура("Цвет, Шрифт", Поле.ЦветТекста, Поле.Шрифт);
	Отбор = Новый("Массив");
	Текст = Новый ТекстовыйДокумент;
КонецПроцедуры
"#;
        check_diagnostics_snapshot_for(
            code,
            DiagnosticCode::StyleElementConstructors,
            expect![[r#""#]],
        );
    }

    #[test]
    fn test_direct_constructor_russian() {
        let code = r#"Процедура ОформитьПоле(Поле)
	Поле.ЦветТекста = Новый Цвет(200, 30, 30);
	Поле.Шрифт = Новый Шрифт(, 12, Истина);
	Поле.Рамка = Новый Рамка(ТипРамкиЭлементаУправления.Одинарная, 1);
КонецПроцедуры
"#;
        check_diagnostics_snapshot_for(
            code,
            DiagnosticCode::StyleElementConstructors,
            expect![[r#"
                StyleElementConstructors @ 2:20..2:43
                  message: Замените конструктор Цвет на получение элемента стиля
                  severity: Error
                StyleElementConstructors @ 3:15..3:40
                  message: Замените конструктор Шрифт на получение элемента стиля
                  severity: Error
                StyleElementConstructors @ 4:15..4:67
                  message: Замените конструктор Рамка на получение элемента стиля
                  severity: Error"#]],
        );
    }

    #[test]
    fn test_string_constructor_russian() {
        let code = r#"ЦветПредупреждения = Новый("Цвет", 250, 180, 0);
ШрифтПримечания = Новый("Шрифт", , 8);
РамкаКарточки = Новый("Рамка", ТипРамкиЭлементаУправления.Двойная);
"#;
        check_diagnostics_snapshot_for(
            code,
            DiagnosticCode::StyleElementConstructors,
            expect![[r#"
                StyleElementConstructors @ 1:22..1:48
                  message: Замените конструктор Цвет на получение элемента стиля
                  severity: Error
                StyleElementConstructors @ 2:19..2:38
                  message: Замените конструктор Шрифт на получение элемента стиля
                  severity: Error
                StyleElementConstructors @ 3:17..3:67
                  message: Замените конструктор Рамка на получение элемента стиля
                  severity: Error"#]],
        );
    }

    #[test]
    fn test_english_constructors() {
        let code = r#"Procedure Decorate(Field)
	Field.TextColor = New Color(10, 120, 60);
	Field.Font = New Font(, 10);
	Field.Border = New Border(ControlBorderType.Single);
EndProcedure
"#;
        check_diagnostics_snapshot_for(
            code,
            DiagnosticCode::StyleElementConstructors,
            expect![[r#"
                StyleElementConstructors @ 2:20..2:42
                  message: Замените конструктор Color на получение элемента стиля
                  severity: Error
                StyleElementConstructors @ 3:15..3:29
                  message: Замените конструктор Font на получение элемента стиля
                  severity: Error
                StyleElementConstructors @ 4:17..4:53
                  message: Замените конструктор Border на получение элемента стиля
                  severity: Error"#]],
        );
    }

    #[test]
    fn test_nested_constructors() {
        let code = r#"Настройки = Новый Соответствие;
Настройки.Вставить("Акцент", Новый ХранилищеЗначения(Новый Цвет(0, 90, 160)));
Настройки.Вставить("Подпись", Новый ХранилищеЗначения(Новый("Шрифт", , 9)));
Настройки.Вставить("Окантовка", Новый("ХранилищеЗначения", Новый Рамка(ТипРамкиЭлементаУправления.Выпуклая)));
"#;
        check_diagnostics_snapshot_for(
            code,
            DiagnosticCode::StyleElementConstructors,
            expect![[r#"
                StyleElementConstructors @ 2:54..2:76
                  message: Замените конструктор Цвет на получение элемента стиля
                  severity: Error
                StyleElementConstructors @ 3:55..3:74
                  message: Замените конструктор Шрифт на получение элемента стиля
                  severity: Error
                StyleElementConstructors @ 4:60..4:108
                  message: Замените конструктор Рамка на получение элемента стиля
                  severity: Error"#]],
        );
    }
}
