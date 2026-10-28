use crate::define_metadata;
use crate::metadata::*;
use crate::{Diagnostic, DiagnosticCode, DiagnosticsContext};
use ide_db::TextRange;
use syntax::{SyntaxKind, SyntaxNode, WalkEvent};

pub const METADATA: DiagnosticMetadata = define_metadata! {
    diagnostic_type: DiagnosticType::Error,
    severity: DiagnosticSeverityLevel::Blocker,
    scope: DiagnosticScope::All,
    modules: &[],
    minutes_to_fix: 5,
    activated_by_default: true,
    compatibility_mode: DiagnosticCompatibilityMode::Undefined,
    tags: &[MetadataTag::Error],
    can_locate_on_project: false,
    extra_min_for_complexity: 0.0,
    lsp_severity_override: "",
};

pub fn check(ctx: &DiagnosticsContext) -> Vec<Diagnostic> {
    let code = DiagnosticCode::CodeBlockBeforeSub;

    if ctx.is_disabled_with_metadata(code) {
        return Vec::new();
    }

    let parse = ctx.parse();
    let root = parse.syntax_node();

    // Methods wrapped in `#Если` are nested inside the directive node, so the
    // scan descends into preprocessor containers instead of looking only at
    // the root's children.
    let mut code_before_sub: Option<TextRange> = None;
    let mut preorder = root.preorder();
    while let Some(event) = preorder.next() {
        let WalkEvent::Enter(node) = event else { continue };
        if is_subroutine(&node) {
            if let Some(range) = code_before_sub {
                return vec![create_diagnostic(range, code, ctx)];
            }
            preorder.skip_subtree();
        } else if is_executable_statement(&node) {
            let range = node.text_range();
            code_before_sub = Some(code_before_sub.map_or(range, |acc| acc.cover(range)));
            preorder.skip_subtree();
        } else if node != root && !is_preproc_container(&node) {
            preorder.skip_subtree();
        }
    }

    Vec::new()
}

fn is_subroutine(node: &SyntaxNode) -> bool {
    matches!(node.kind(), SyntaxKind::PROCEDURE_DEF | SyntaxKind::FUNCTION_DEF)
}

fn is_preproc_container(node: &SyntaxNode) -> bool {
    matches!(
        node.kind(),
        SyntaxKind::PRE_IF_DIR
            | SyntaxKind::PRE_ELSIF_CLAUSE
            | SyntaxKind::PRE_ELSE_CLAUSE
            | SyntaxKind::PRE_TORN_IF_PART
            | SyntaxKind::PRE_REGION_DIR
    )
}

fn create_diagnostic(
    range: TextRange,
    code: DiagnosticCode,
    ctx: &DiagnosticsContext,
) -> Diagnostic {
    Diagnostic {
        code,
        message: "Обнаружен блок кода перед объявлением процедур и функций".to_string(),
        severity: ctx.severity(code),
        range,
        tags: ctx.tags(code),
        fixes: vec![],
    }
}

fn is_executable_statement(node: &SyntaxNode) -> bool {
    matches!(
        node.kind(),
        SyntaxKind::CALL_STMT
            | SyntaxKind::ASSIGN_STMT
            | SyntaxKind::IF_STMT
            | SyntaxKind::WHILE_STMT
            | SyntaxKind::FOR_STMT
            | SyntaxKind::FOR_EACH_STMT
            | SyntaxKind::TRY_STMT
            | SyntaxKind::RETURN_STMT
            | SyntaxKind::RAISE_STMT
            | SyntaxKind::EXECUTE_STMT
            | SyntaxKind::BREAK_STMT
            | SyntaxKind::CONTINUE_STMT
            | SyntaxKind::GOTO_STMT
            | SyntaxKind::LABEL_STMT
            | SyntaxKind::ADD_HANDLER_STMT
            | SyntaxKind::REMOVE_HANDLER_STMT
    )
}

#[cfg(test)]
mod tests {
    use crate::test_utils::check_diagnostics_snapshot_for;
    use crate::DiagnosticCode;
    use expect_test::expect;

    fn check(code: &str, expected: expect_test::Expect) {
        check_diagnostics_snapshot_for(code, DiagnosticCode::CodeBlockBeforeSub, expected);
    }

    #[test]
    fn test_valid_order() {
        let code = r#"Перем КаталогКниг;
Перем ЧислоВыдач;

Процедура ЗагрузитьКаталог()
	КаталогКниг = Новый Массив;
КонецПроцедуры

ЧислоВыдач = 0;
ЗагрузитьКаталог();
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_code_before_procedure() {
        let code = r#"Перем КаталогКниг;
Перем ЧислоВыдач;
ЧислоВыдач = 0;

Процедура ЗагрузитьКаталог()
	КаталогКниг = Новый Массив;
КонецПроцедуры

ЗагрузитьКаталог();
"#;
        check(
            code,
            expect![[r#"
            CodeBlockBeforeSub @ 3:1..3:15
              message: Обнаружен блок кода перед объявлением процедур и функций
              severity: Blocker"#]],
        );
    }

    #[test]
    fn test_code_between_two_methods() {
        let code = r#"Процедура ВыдатьКнигу()
КонецПроцедуры

ЧислоВыдач = 0;

Процедура ВернутьКнигу()
КонецПроцедуры
"#;
        check(
            code,
            expect![[r#"
            CodeBlockBeforeSub @ 4:1..4:15
              message: Обнаружен блок кода перед объявлением процедур и функций
              severity: Blocker"#]],
        );
    }

    #[test]
    fn test_body_after_all_methods_is_valid() {
        let code = r#"Процедура ВыдатьКнигу()
КонецПроцедуры

Процедура ВернутьКнигу()
КонецПроцедуры

ЧислоВыдач = 0;
ВыдатьКнигу();
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_code_before_method_inside_if() {
        let code = r#"Процедура ВыдатьКнигу()
КонецПроцедуры

ЧислоВыдач = 0;

#Если Сервер Тогда
Процедура ВернутьКнигу()
КонецПроцедуры
#КонецЕсли
"#;
        check(
            code,
            expect![[r#"
            CodeBlockBeforeSub @ 4:1..4:15
              message: Обнаружен блок кода перед объявлением процедур и функций
              severity: Blocker"#]],
        );
    }

    #[test]
    fn test_range_excludes_method_inside_if() {
        let code = r#"Процедура ВыдатьКнигу()
КонецПроцедуры
#Если Сервер Тогда
Процедура ВернутьКнигу()
КонецПроцедуры
ЧислоВыдач = 0;
#КонецЕсли
Процедура СписатьВетхие()
КонецПроцедуры
"#;
        check(
            code,
            expect![[r#"
            CodeBlockBeforeSub @ 6:1..6:15
              message: Обнаружен блок кода перед объявлением процедур и функций
              severity: Blocker"#]],
        );
    }

    #[test]
    fn test_execute_between_methods() {
        let code = r#"Процедура ВыдатьКнигу()
КонецПроцедуры

Выполнить("ЧислоВыдач = 0;");

Процедура ВернутьКнигу()
КонецПроцедуры
"#;
        check(
            code,
            expect![[r#"
            CodeBlockBeforeSub @ 4:1..4:30
              message: Обнаружен блок кода перед объявлением процедур и функций
              severity: Blocker"#]],
        );
    }

    #[test]
    fn test_multiple_code_blocks() {
        let code = r#"Перем Читатели;

Читатели = Новый Соответствие;
ОбновитьАбонементы(Читатели);
Читатели.Удалить("К-17");

Функция ОбновитьАбонементы(Список)
	Возврат Список.Количество();
КонецФункции
"#;
        check(
            code,
            expect![[r#"
            CodeBlockBeforeSub @ 3:1..5:25
              message: Обнаружен блок кода перед объявлением процедур и функций
              severity: Blocker"#]],
        );
    }

    #[test]
    fn test_no_procedures() {
        let code = r#"Перем Полка;

Полка = Новый Массив;
Полка.Добавить("Атлас");
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_english_keywords() {
        let code = r#"Var Shelf;

Shelf = New Array;

Function ShelfSize()
	Return Shelf.Count();
EndFunction
"#;
        check(
            code,
            expect![[r#"
            CodeBlockBeforeSub @ 3:1..3:18
              message: Обнаружен блок кода перед объявлением процедур и функций
              severity: Blocker"#]],
        );
    }

    #[test]
    fn test_code_inside_region_before_sub() {
        let code = r#"Перем ЖурналВыдачи;

#Область Подготовка
ЖурналВыдачи = Новый Массив;
ОтметитьОткрытие(ЖурналВыдачи);
#КонецОбласти

Процедура ОтметитьОткрытие(Журнал)
	Журнал.Добавить(ТекущаяДата());
КонецПроцедуры

#Область ТелоМодуля
ЖурналВыдачи.Очистить();
#КонецОбласти
"#;
        check(
            code,
            expect![[r#"
            CodeBlockBeforeSub @ 4:1..5:31
              message: Обнаружен блок кода перед объявлением процедур и функций
              severity: Blocker"#]],
        );
    }

    #[test]
    fn test_regions_holding_only_methods_are_silent() {
        let code = r#"#Область ПрограммныйИнтерфейс

&НаСервере
Функция СвободныеЭкземпляры(Книга)
	Возврат Книга.Экземпляры.Количество();
КонецФункции

&НаКлиенте
Процедура ПоказатьКарточку(Книга)
КонецПроцедуры

#КонецОбласти

&НаСервере
Процедура СписатьВетхие()
КонецПроцедуры
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_code_before_region_wrapped_method() {
        let code = r#"ПроверитьФонд();

#Область СлужебныеПроцедурыИФункции
Процедура ПроверитьФонд()
КонецПроцедуры
#КонецОбласти

Процедура Инвентаризация()
КонецПроцедуры
"#;
        check(
            code,
            expect![[r#"
            CodeBlockBeforeSub @ 1:1..1:16
              message: Обнаружен блок кода перед объявлением процедур и функций
              severity: Blocker"#]],
        );
    }

    #[test]
    fn test_code_in_outer_region_before_nested_method() {
        let code = r#"#Область Фонд
Стеллажи = Новый Соответствие;

#Область Перестановка
Процедура ПереставитьСтеллаж()
КонецПроцедуры
#КонецОбласти
#КонецОбласти

Процедура Инвентаризация()
КонецПроцедуры
"#;
        check(
            code,
            expect![[r#"
            CodeBlockBeforeSub @ 2:1..2:30
              message: Обнаружен блок кода перед объявлением процедур и функций
              severity: Blocker"#]],
        );
    }
}
