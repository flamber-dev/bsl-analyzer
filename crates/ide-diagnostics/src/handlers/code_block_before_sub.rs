use crate::define_metadata;
use crate::metadata::*;
use crate::{Diagnostic, DiagnosticCode, DiagnosticsContext};
use syntax::{SyntaxKind, SyntaxNode};

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

    let mut code_blocks_before_sub: Vec<SyntaxNode> = Vec::new();

    for child in root.children() {
        if is_subroutine(&child) {
            if !code_blocks_before_sub.is_empty() {
                return vec![create_diagnostic(&code_blocks_before_sub, code, ctx)];
            }
            break;
        }

        if is_code_block(&child) {
            code_blocks_before_sub.push(child.clone());
        }
    }

    Vec::new()
}

fn is_subroutine(node: &SyntaxNode) -> bool {
    matches!(node.kind(), SyntaxKind::PROCEDURE_DEF | SyntaxKind::FUNCTION_DEF)
}

fn is_code_block(node: &SyntaxNode) -> bool {
    match node.kind() {
        SyntaxKind::CALL_STMT
        | SyntaxKind::ASSIGN_STMT
        | SyntaxKind::IF_STMT
        | SyntaxKind::WHILE_STMT
        | SyntaxKind::FOR_STMT
        | SyntaxKind::FOR_EACH_STMT
        | SyntaxKind::TRY_STMT
        | SyntaxKind::RETURN_STMT
        | SyntaxKind::RAISE_STMT
        | SyntaxKind::BREAK_STMT
        | SyntaxKind::CONTINUE_STMT => true,

        SyntaxKind::PRE_REGION_DIR | SyntaxKind::PRE_IF_DIR => contains_executable_code(node),

        SyntaxKind::VAR_DEF
        | SyntaxKind::WHITESPACE
        | SyntaxKind::NEWLINE
        | SyntaxKind::COMMENT
        | SyntaxKind::COMPILER_DIRECTIVE
        | SyntaxKind::ANNOTATION => false,

        _ => {
            tracing::debug!(
                kind = ?node.kind(),
                "Unexpected node kind in SourceFile children"
            );
            false
        }
    }
}

fn contains_executable_code(node: &SyntaxNode) -> bool {
    for child in node.children() {
        if is_subroutine(&child) {
            continue;
        }
        if matches!(
            child.kind(),
            SyntaxKind::CALL_STMT
                | SyntaxKind::ASSIGN_STMT
                | SyntaxKind::IF_STMT
                | SyntaxKind::WHILE_STMT
                | SyntaxKind::FOR_STMT
                | SyntaxKind::FOR_EACH_STMT
                | SyntaxKind::TRY_STMT
                | SyntaxKind::RETURN_STMT
                | SyntaxKind::RAISE_STMT
                | SyntaxKind::BREAK_STMT
                | SyntaxKind::CONTINUE_STMT
        ) {
            return true;
        }
        if contains_executable_code(&child) {
            return true;
        }
    }
    false
}

fn create_diagnostic(
    code_blocks: &[SyntaxNode],
    code: DiagnosticCode,
    ctx: &DiagnosticsContext,
) -> Diagnostic {
    use ide_db::TextRange;

    let first = code_blocks.first().unwrap();
    let last = code_blocks.last().unwrap();

    let start_offset = if first.kind() == SyntaxKind::PRE_REGION_DIR {
        first
            .descendants()
            .find(is_executable_statement)
            .map(|n| n.text_range().start())
            .unwrap_or_else(|| first.text_range().start())
    } else {
        first.text_range().start()
    };

    let end_offset = last.text_range().end();
    let range = TextRange::new(start_offset, end_offset);

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
            | SyntaxKind::BREAK_STMT
            | SyntaxKind::CONTINUE_STMT
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
