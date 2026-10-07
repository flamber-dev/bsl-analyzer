use crate::define_metadata;
use crate::metadata::*;
use crate::{BodyContext, Diagnostic, DiagnosticCode};
use hir::BodySourceMap;
use hir::LocalRange;

pub const METADATA: DiagnosticMetadata = define_metadata! {
    diagnostic_type: DiagnosticType::CodeSmell,
    severity: DiagnosticSeverityLevel::Minor,
    scope: DiagnosticScope::All,
    modules: &[],
    minutes_to_fix: 5,
    activated_by_default: true,
    compatibility_mode: DiagnosticCompatibilityMode::Undefined,
    tags: &[MetadataTag::Brainoverload],
    can_locate_on_project: false,
    extra_min_for_complexity: 0.0,
    lsp_severity_override: "",
};

const DEFAULT_MAX_IF_CONDITION_COMPLEXITY: i64 = 3;

pub fn check_body(ctx: &BodyContext, acc: &mut Vec<Diagnostic<LocalRange>>) {
    let code = DiagnosticCode::IfConditionComplexity;
    if ctx.is_disabled_with_metadata(code) {
        return;
    }

    let max_complexity =
        ctx.config_int(code, "maxIfConditionComplexity", DEFAULT_MAX_IF_CONDITION_COMPLEXITY)
            as u32;
    let metrics = ctx.hir_metrics();
    if metrics.if_conditions.is_empty() {
        return;
    }
    emit_conditions(ctx, code, &metrics, ctx.source_map(), max_complexity, acc);
}

fn emit_conditions(
    ctx: &BodyContext,
    code: DiagnosticCode,
    metrics: &hir::metrics::HirMethodMetrics,
    source_map: &BodySourceMap,
    max_complexity: u32,
    out: &mut Vec<Diagnostic<LocalRange>>,
) {
    for cond in metrics.if_conditions.iter() {
        let complexity = cond.logical_op_count + 1;
        if complexity <= max_complexity {
            continue;
        }
        // Подрезать хвостовой пробел не нужно: узел им не кончается.
        let Some(range) = source_map.expr_range(cond.condition) else { continue };
        out.push(Diagnostic {
            code,
            message: format!(
                "Условие имеет сложность {} (максимум {}). Упростите условие или вынесите части в переменные.",
                complexity, max_complexity
            ),
            severity: ctx.severity(code),
            range,
            tags: ctx.tags(code),
            fixes: vec![],
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::test_utils::*;
    use crate::{DiagnosticCode, DiagnosticsConfig};
    use expect_test::expect;

    fn check(code: &str, expected: expect_test::Expect) {
        check_diagnostics_snapshot_for(code, DiagnosticCode::IfConditionComplexity, expected);
    }

    #[test]
    fn test_simple_condition() {
        let code = r#"Процедура Полить(Грядка)
	Если Грядка.Сухая ИЛИ Грядка.Жарко Тогда
		Грядка.Полить();
	КонецЕсли;
КонецПроцедуры
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_at_threshold() {
        // Complexity is the number of logical operators plus one: two operators is 3.
        let code = r#"Процедура Полить(Грядка)
	Если Грядка.Сухая И Грядка.Засеяна ИЛИ Грядка.Жарко Тогда
		Грядка.Полить();
	КонецЕсли;
КонецПроцедуры
"#;
        check(code, expect![""]);
    }

    #[test]
    fn test_complex_condition() {
        let code = r#"Процедура Полить(Грядка)
	Если Грядка.Сухая И Грядка.Засеяна ИЛИ Грядка.Жарко И Грядка.Теплица Тогда
		Грядка.Полить();
	КонецЕсли;
КонецПроцедуры
"#;
        check(
            code,
            expect![[r#"
                IfConditionComplexity @ 2:7..2:70
                  message: Условие имеет сложность 4 (максимум 3). Упростите условие или вынесите части в переменные.
                  severity: Information"#]],
        );
    }

    #[test]
    fn test_elseif_complex() {
        let code = r#"Процедура Полить(Грядка)
	Если Грядка.Дождь Тогда
		Возврат;
	ИначеЕсли Грядка.Сухая И Грядка.Засеяна ИЛИ Грядка.Жарко И Грядка.Теплица Тогда
		Грядка.Полить();
	КонецЕсли;
КонецПроцедуры
"#;
        check(
            code,
            expect![[r#"
                IfConditionComplexity @ 4:12..4:75
                  message: Условие имеет сложность 4 (максимум 3). Упростите условие или вынесите части в переменные.
                  severity: Information"#]],
        );
    }

    #[test]
    fn test_english_condition() {
        let code = r#"Procedure Water(Bed)
	If Bed.Dry And Bed.Sown Or Bed.Hot And Bed.Greenhouse Then
		Bed.Water();
	EndIf;
EndProcedure
"#;
        check(
            code,
            expect![[r#"
                IfConditionComplexity @ 2:5..2:55
                  message: Условие имеет сложность 4 (максимум 3). Упростите условие или вынесите части в переменные.
                  severity: Information"#]],
        );
    }

    #[test]
    fn test_large_multiline_condition() {
        let code = r#"Функция ЭтоОвощ(Культура)
	Если Культура = "Морковь"
		ИЛИ Культура = "Свекла"
		ИЛИ Культура = "Капуста"
		ИЛИ Культура = "Лук"
		ИЛИ Культура = "Чеснок"
		ИЛИ Культура = "Редис" Тогда
		Возврат Истина;
	КонецЕсли;
	Возврат Ложь;
КонецФункции
"#;
        check(
            code,
            expect![[r#"
            IfConditionComplexity @ 2:7..7:25
              message: Условие имеет сложность 6 (максимум 3). Упростите условие или вынесите части в переменные.
              severity: Information"#]],
        );
    }

    #[test]
    fn test_nested_outer_pass_inner_warn() {
        let code = r#"Процедура Рассадить(Культура)
	Если Культура = "Томат"
		ИЛИ Культура = "Перец" Тогда
		Если Культура.Сорт = "Ранний"
			ИЛИ Культура.Сорт = "Средний"
			ИЛИ Культура.Сорт = "Поздний"
			ИЛИ Культура.Сорт = "Гибрид" Тогда
			Возврат;
		КонецЕсли;
	КонецЕсли;
КонецПроцедуры
"#;
        check(
            code,
            expect![[r#"
            IfConditionComplexity @ 4:8..7:32
              message: Условие имеет сложность 4 (максимум 3). Упростите условие или вынесите части в переменные.
              severity: Information"#]],
        );
    }

    #[test]
    fn test_if_and_elseif_both_complex() {
        let code = r#"Функция Сезон(Месяц)
	Если Месяц = 12
		ИЛИ Месяц = 1
		ИЛИ Месяц = 2
		ИЛИ Месяц = 13 Тогда
		Возврат "Зима";
	ИначеЕсли Месяц = 3
		ИЛИ Месяц = 4
		ИЛИ Месяц = 5
		ИЛИ Месяц = 6
		ИЛИ Месяц = 7 Тогда
		Возврат "Тепло";
	Иначе
		Возврат "Осень";
	КонецЕсли;
КонецФункции
"#;
        check(
            code,
            expect![[r#"
            IfConditionComplexity @ 2:7..5:17
              message: Условие имеет сложность 4 (максимум 3). Упростите условие или вынесите части в переменные.
              severity: Information
            IfConditionComplexity @ 7:12..11:16
              message: Условие имеет сложность 5 (максимум 3). Упростите условие или вынесите части в переменные.
              severity: Information"#]],
        );
    }

    #[test]
    fn test_sub_default_threshold_emits() {
        let code = r#"Процедура Полить(Грядка)
	Если Грядка.Сухая ИЛИ Грядка.Жарко Тогда
		Грядка.Полить();
	КонецЕсли;
КонецПроцедуры
"#;
        let mut config = DiagnosticsConfig::default();
        config.parameters.insert(
            DiagnosticCode::IfConditionComplexity,
            serde_json::json!({ "maxIfConditionComplexity": 1 }),
        );
        let diagnostics: Vec<_> =
            check_hir_diagnostic_with_config(code, config, crate::diagnostics)
                .into_iter()
                .filter(|d| d.code == DiagnosticCode::IfConditionComplexity)
                .collect();
        expect![[r#"
            IfConditionComplexity @ 2:7..2:36
              message: Условие имеет сложность 2 (максимум 1). Упростите условие или вынесите части в переменные.
              severity: Information"#]].assert_eq(&format_diags(code, &diagnostics));
    }
}
