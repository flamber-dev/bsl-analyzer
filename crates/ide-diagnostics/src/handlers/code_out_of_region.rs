use crate::define_metadata;
use crate::metadata::*;
use crate::{Diagnostic, DiagnosticCode, DiagnosticsContext};
use hir::module_structure::significant::is_significant_for_code_out_of_region;
use hir::RegionTree;
use syntax::{ast, ast::AstNode, SyntaxKind, SyntaxNode};

pub const METADATA: DiagnosticMetadata = define_metadata! {
    diagnostic_type: DiagnosticType::CodeSmell,
    severity: DiagnosticSeverityLevel::Info,
    scope: DiagnosticScope::Bsl,
    modules: &[],
    minutes_to_fix: 1,
    activated_by_default: true,
    compatibility_mode: DiagnosticCompatibilityMode::Compatibility8320,
    tags: &[MetadataTag::Standard],
    can_locate_on_project: false,
    extra_min_for_complexity: 0.0,
    lsp_severity_override: "",
};

pub fn check(ctx: &DiagnosticsContext) -> Vec<Diagnostic> {
    let code = DiagnosticCode::CodeOutOfRegion;

    if ctx.is_disabled_with_metadata(code) {
        return Vec::new();
    }

    let parse = ctx.parse();
    let root = parse.syntax_node();

    let region_tree = ctx.region_tree();

    let mut diagnostics = Vec::new();
    check_node(&root, &region_tree, code, ctx, &mut diagnostics);

    // A module with no module-level regions is a single structural problem:
    // report it once, on the first out-of-region element, instead of nagging
    // on every module-level statement, variable and method.
    if region_tree.module_level_regions().next().is_none() {
        diagnostics.truncate(1);
    }

    diagnostics
}

/// Диапазон узла вместе с его точкой с запятой.
///
/// Конец берётся покрытием самой найденной `;`, а не сдвигом конца узла на
/// байт: между узлом и его точкой с запятой стоит тривия, и арифметика от
/// конца узла оборвала бы диапазон внутри пробела.
fn range_with_semicolon(node: &SyntaxNode) -> ide_db::TextRange {
    let base_range = node.text_range();

    syntax::trailing_semicolon(node)
        .map_or(base_range, |token| base_range.cover(token.text_range()))
}

fn check_node(
    node: &SyntaxNode,
    region_tree: &RegionTree,
    code: DiagnosticCode,
    ctx: &DiagnosticsContext,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for child in node.children() {
        if matches!(
            child.kind(),
            SyntaxKind::PRE_IF_DIR | SyntaxKind::PRE_ELSE_CLAUSE | SyntaxKind::PRE_ELSIF_CLAUSE
        ) {
            check_node(&child, region_tree, code, ctx, diagnostics);
            continue;
        }

        if is_module_level_element(&child)
            && is_significant_for_code_out_of_region(&child)
            && !region_tree.is_range_inside_region(child.text_range())
        {
            let (element_type, range) = match child.kind() {
                SyntaxKind::FUNCTION_DEF => {
                    let range = ast::FunctionDef::cast(child.clone())
                        .and_then(|f| f.name())
                        .map(|name| name.text_range())
                        .unwrap_or_else(|| child.text_range());
                    ("Функция", range)
                }
                SyntaxKind::PROCEDURE_DEF => {
                    let range = ast::ProcedureDef::cast(child.clone())
                        .and_then(|p| p.name())
                        .map(|name| name.text_range())
                        .unwrap_or_else(|| child.text_range());
                    ("Процедура", range)
                }
                SyntaxKind::VAR_DEF => ("Переменная", child.text_range()),
                _ => ("Элемент кода", range_with_semicolon(&child)),
            };

            tracing::debug!(
                kind = ?child.kind(),
                range = ?range,
                text = %child.text().to_string().lines().next().unwrap_or(""),
                "CodeOutOfRegion: found element outside region"
            );

            diagnostics.push(Diagnostic {
                code,
                message: format!(
                    "{} находится вне области (#Область/#Region). \
                     Весь код модуля должен быть организован в области для лучшей структуры.",
                    element_type
                ),
                severity: ctx.severity(code),
                range,
                tags: ctx.tags(code),
                fixes: vec![],
            });
        }
    }
}

fn is_module_level_element(node: &SyntaxNode) -> bool {
    let parent = match node.parent() {
        Some(p) => p,
        None => return false,
    };

    match parent.kind() {
        SyntaxKind::SOURCE_FILE | SyntaxKind::PRE_ELSE_CLAUSE | SyntaxKind::PRE_ELSIF_CLAUSE => {
            true
        }
        SyntaxKind::PRE_IF_DIR => {
            if matches!(node.kind(), SyntaxKind::CALL_STMT | SyntaxKind::ASSIGN_STMT) {
                has_preceding_definition(&parent, node)
            } else {
                true
            }
        }
        _ => false,
    }
}

fn has_preceding_definition(parent: &SyntaxNode, node: &SyntaxNode) -> bool {
    let node_start = node.text_range().start();
    for sibling in parent.children() {
        if sibling.text_range().start() < node_start
            && matches!(
                sibling.kind(),
                SyntaxKind::VAR_DEF
                    | SyntaxKind::PROCEDURE_DEF
                    | SyntaxKind::FUNCTION_DEF
                    | SyntaxKind::PRE_REGION_DIR
            )
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use crate::test_utils::check_diagnostics_snapshot_for;
    use crate::DiagnosticCode;
    use expect_test::expect;

    fn check(code: &str, expected: expect_test::Expect) {
        check_diagnostics_snapshot_for(code, DiagnosticCode::CodeOutOfRegion, expected);
    }

    #[test]
    fn test_module_variables_outside_region() {
        let code = r#"// Метеостанция: переменные модуля
#Если Сервер Тогда
Перем Датчики;
#Область ПеременныеМодуля
Перем ИнтервалОпроса;
#КонецОбласти
Перем ПоследнийЗамер;
#КонецЕсли
"#;
        check(
            code,
            expect![[r#"
            CodeOutOfRegion @ 3:1..3:15
              message: Переменная находится вне области (#Область/#Region). Весь код модуля должен быть организован в области для лучшей структуры.
              severity: Hint
            CodeOutOfRegion @ 7:1..7:22
              message: Переменная находится вне области (#Область/#Region). Весь код модуля должен быть организован в области для лучшей структуры.
              severity: Hint"#]],
        );
    }

    #[test]
    fn test_methods_outside_region_with_inner_regions() {
        let code = r#"#Область ПрограммныйИнтерфейс
Функция ТекущаяТемпература() Экспорт
	Возврат 0;
КонецФункции
#КонецОбласти

#Если Клиент Тогда
#Иначе
Процедура Откалибровать()
	#Область Шаги
	СброситьДатчики();
	#КонецОбласти
КонецПроцедуры
#КонецЕсли

Функция Влажность()
	#Область Расчет
	Возврат 55;
	#КонецОбласти
КонецФункции
"#;
        check(
            code,
            expect![[r#"
            CodeOutOfRegion @ 9:11..9:24
              message: Процедура находится вне области (#Область/#Region). Весь код модуля должен быть организован в области для лучшей структуры.
              severity: Hint
            CodeOutOfRegion @ 16:9..16:18
              message: Функция находится вне области (#Область/#Region). Весь код модуля должен быть организован в области для лучшей структуры.
              severity: Hint"#]],
        );
    }

    #[test]
    fn test_module_body_statements() {
        let code = r#"//////////////////////////
// Опрос датчиков
//////////////////////////
#Если Сервер Тогда
#Область СлужебныеПроцедурыИФункции
Процедура Опросить()
КонецПроцедуры
#КонецОбласти
#КонецЕсли

#Область Инициализация
ИнтервалОпроса = 60;
Если ИнтервалОпроса > 0 Тогда
	Опросить();
КонецЕсли;
#КонецОбласти

ПоследнийЗамер = ТекущаяДата();

#Область Журнал
	Записей = 0;
#КонецОбласти
Записей = Записей + 1;

Если Записей > 10 Тогда
#Если Сервер Тогда
Опросить();
#ИначеЕсли Клиент Тогда
Записей = 0;
#Иначе
#Область ВеткаПоУмолчанию
Опросить();
#КонецОбласти
#КонецЕсли
КонецЕсли;
"#;
        check(
            code,
            expect![[r#"
            CodeOutOfRegion @ 18:1..18:32
              message: Элемент кода находится вне области (#Область/#Region). Весь код модуля должен быть организован в области для лучшей структуры.
              severity: Hint
            CodeOutOfRegion @ 23:1..23:23
              message: Элемент кода находится вне области (#Область/#Region). Весь код модуля должен быть организован в области для лучшей структуры.
              severity: Hint
            CodeOutOfRegion @ 25:1..35:11
              message: Элемент кода находится вне области (#Область/#Region). Весь код модуля должен быть организован в области для лучшей структуры.
              severity: Hint"#]],
        );
    }

    #[test]
    fn test_empty_file() {
        check("", expect![[r#""#]]);
    }

    #[test]
    fn test_comment_only_module() {
        check("// Метеостанция: модуль пока пуст\n\n", expect![[r#""#]]);
    }

    #[test]
    fn test_no_regions() {
        let code = r#"// Модуль без областей

Перем ШкалаДавления;

Функция ДавлениеВГектопаскалях(Миллиметры)
	Возврат Миллиметры * 1.333;
КонецФункции

Процедура ВыбратьШкалу()
	ШкалаДавления = "гПа";
КонецПроцедуры

ВыбратьШкалу();
"#;
        check(
            code,
            expect![[r#"
            CodeOutOfRegion @ 3:1..3:21
              message: Переменная находится вне области (#Область/#Region). Весь код модуля должен быть организован в области для лучшей структуры.
              severity: Hint"#]],
        );
    }

    #[test]
    fn test_region_less_module_collapses_to_single_finding() {
        let code = r#"Функция ТочкаРосы(Температура, Влажность)
	Возврат Температура - (100 - Влажность) / 5;
КонецФункции

Процедура ОчиститьАрхив()
КонецПроцедуры
"#;
        check(
            code,
            expect![[r#"
            CodeOutOfRegion @ 1:9..1:18
              message: Функция находится вне области (#Область/#Region). Весь код модуля должен быть организован в области для лучшей структуры.
              severity: Hint"#]],
        );
    }

    #[test]
    fn test_standard_preproc() {
        let code = r#"#Если Сервер Или ВнешнееСоединение Тогда
#Область ПрограммныйИнтерфейс
#КонецОбласти
#Иначе
ВызватьИсключение НСтр("ru = 'Модуль доступен только на сервере'");
#КонецЕсли
"#;
        check(code, expect![[r#""#]]);
    }

    /// An off-canon closing directive must close the region. Mistaken for an
    /// opening one, it leaves the region open to EOF and everything after it
    /// silently counts as regioned code.
    #[test]
    fn code_after_off_canon_closing_directive_is_out_of_region() {
        let code = r#"#Область Опрос
Процедура СнятьПоказания()
КонецПроцедуры
#конецОбласти

Процедура ОтправитьСводку()
КонецПроцедуры
"#;
        check(
            code,
            expect![[r#"
            CodeOutOfRegion @ 6:11..6:26
              message: Процедура находится вне области (#Область/#Region). Весь код модуля должен быть организован в области для лучшей структуры.
              severity: Hint"#]],
        );
    }

    #[test]
    fn test_single_method_module() {
        check(
            "\nФункция ИмяСтанции()\n\tВозврат \"Север-3\";\nКонецФункции\n",
            expect![[r#"
            CodeOutOfRegion @ 2:9..2:19
              message: Функция находится вне области (#Область/#Region). Весь код модуля должен быть организован в области для лучшей структуры.
              severity: Hint"#]],
        );
    }

    #[test]
    fn test_code_block() {
        check(
            "ОбновитьТабло(\"Ясно\");",
            expect![[r#"
            CodeOutOfRegion @ 1:1..1:23
              message: Элемент кода находится вне области (#Область/#Region). Весь код модуля должен быть организован в области для лучшей структуры.
              severity: Hint"#]],
        );
    }

    #[test]
    fn test_code_in_region() {
        let code = r#"#Область ОбработчикиСобытий

Процедура ПриПолученииСводки(Сводка) Экспорт
	Сводка.Принята = Истина;
КонецПроцедуры

#КонецОбласти
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_code_outside_region() {
        let code = r#"Процедура ПриПолученииСводки(Сводка) Экспорт
	Сводка.Принята = Истина;
КонецПроцедуры
"#;
        check(
            code,
            expect![[r#"
            CodeOutOfRegion @ 1:11..1:29
              message: Процедура находится вне области (#Область/#Region). Весь код модуля должен быть организован в области для лучшей структуры.
              severity: Hint"#]],
        );
    }

    #[test]
    fn test_goto_stmt_outside_region_snapshot() {
        check(
            "Перейти ~Повтор;",
            expect![[r#"
            CodeOutOfRegion @ 1:1..1:17
              message: Элемент кода находится вне области (#Область/#Region). Весь код модуля должен быть организован в области для лучшей структуры.
              severity: Hint"#]],
        );
    }

    #[test]
    fn test_label_stmt_outside_region_snapshot() {
        check("~Повтор:", expect![[r#""#]]);
    }

    #[test]
    fn test_execute_stmt_outside_region_snapshot() {
        check(
            "Выполнить(ТекстКоманды);",
            expect![[r#"
            CodeOutOfRegion @ 1:1..1:25
              message: Элемент кода находится вне области (#Область/#Region). Весь код модуля должен быть организован в области для лучшей структуры.
              severity: Hint"#]],
        );
    }

    #[test]
    fn test_add_handler_stmt_outside_region_snapshot() {
        check(
            "ДобавитьОбработчик Станция.ПриОтказе, ОбработатьОтказ;",
            expect![[r#"
            CodeOutOfRegion @ 1:1..1:55
              message: Элемент кода находится вне области (#Область/#Region). Весь код модуля должен быть организован в области для лучшей структуры.
              severity: Hint"#]],
        );
    }

    #[test]
    fn test_remove_handler_stmt_outside_region_snapshot() {
        check(
            "УдалитьОбработчик Станция.ПриОтказе, ОбработатьОтказ;",
            expect![[r#"
            CodeOutOfRegion @ 1:1..1:54
              message: Элемент кода находится вне области (#Область/#Region). Весь код модуля должен быть организован в области для лучшей структуры.
              severity: Hint"#]],
        );
    }

    #[test]
    fn test_standalone_raise_stmt_outside_region_snapshot() {
        check("ВызватьИсключение;", expect![[r#""#]]);
    }

    #[test]
    fn test_pre_region_dir_covers_inner_code_but_not_following_stmt_snapshot() {
        let code = r#"#Область ЗапускОпроса
ЗапуститьОпрос(15);
#КонецОбласти

ОстановитьОпрос();
"#;
        check(
            code,
            expect![[r#"
            CodeOutOfRegion @ 5:1..5:19
              message: Элемент кода находится вне области (#Область/#Region). Весь код модуля должен быть организован в области для лучшей структуры.
              severity: Hint"#]],
        );
    }
}
