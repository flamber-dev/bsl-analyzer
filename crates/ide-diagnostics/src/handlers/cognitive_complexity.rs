use crate::define_metadata;
use crate::metadata::*;
use crate::{BodyContext, Diagnostic, DiagnosticCode};
use hir::LocalRange;

pub const METADATA: DiagnosticMetadata = define_metadata! {
    diagnostic_type: DiagnosticType::CodeSmell,
    severity: DiagnosticSeverityLevel::Critical,
    scope: DiagnosticScope::All,
    modules: &[],
    minutes_to_fix: 15,
    activated_by_default: true,
    compatibility_mode: DiagnosticCompatibilityMode::Undefined,
    tags: &[MetadataTag::Brainoverload],
    can_locate_on_project: false,
    extra_min_for_complexity: 1.0,
    lsp_severity_override: "",
};

pub fn check_body(ctx: &BodyContext, acc: &mut Vec<Diagnostic<LocalRange>>) {
    let code = DiagnosticCode::CognitiveComplexity;
    if ctx.is_disabled_with_metadata(code) {
        return;
    }

    let threshold = ctx.config_int(code, "complexityThreshold", 15) as u32;
    let (Some(decl), Some(name_range)) = (ctx.decl(), ctx.method_name_range()) else {
        return;
    };
    let metrics = ctx.hir_metrics();
    let recursion_bonus =
        if ctx.module_recursive_methods().contains(&decl.id.local_id) { 1 } else { 0 };
    let total = metrics.cognitive + recursion_bonus;
    if total <= threshold {
        return;
    }
    let method_type = if decl.is_function { "Функция" } else { "Процедура" };
    acc.push(Diagnostic {
        code,
        message: format!(
            "{} '{}' имеет когнитивную сложность {} (максимум: {}). \
             Упростите логику или уменьшите вложенность",
            method_type,
            decl.name.as_str(),
            total,
            threshold
        ),
        severity: ctx.severity(code),
        range: name_range,
        tags: ctx.tags(code),
        fixes: vec![],
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{
        check_diagnostics_snapshot_for, check_hir_diagnostic_with_config, format_diags,
    };
    use crate::DiagnosticsConfig;
    use expect_test::expect;
    use hir::ModuleId;
    use ide_db::base_db::{SourceDatabase, SourceRoot, SourceRootId};
    use ide_db::vfs::{FileSet, VfsPath};
    use ide_db::{RootDatabase, RootDatabaseImpl};
    use std::rc::Rc;
    use test_fixture::Fixture;

    fn check_with_threshold(code: &str, threshold: i64, expected: expect_test::Expect) {
        let mut config = DiagnosticsConfig::default();
        config.parameters.insert(
            DiagnosticCode::CognitiveComplexity,
            serde_json::json!({ "complexityThreshold": threshold }),
        );
        let diagnostics: Vec<_> =
            check_hir_diagnostic_with_config(code, config, crate::diagnostics)
                .into_iter()
                .filter(|d| d.code == DiagnosticCode::CognitiveComplexity)
                .collect();
        expected.assert_eq(&format_diags(code, &diagnostics));
    }

    /// Counted by hand with the current formula (`hir-def` metrics): a structure adds
    /// 1 + nesting, every `И`/`ИЛИ` adds 1, `ИначеЕсли`/`Иначе` add 1 and nest their
    /// bodies two levels below the `Если`. Если 1, Для 1, Если 2 (nesting 1),
    /// Пока 3 (nesting 2), ИначеЕсли 1, `И` 1, Иначе 1, Исключение 4 (inside Иначе:
    /// nesting 3), `?()` 1 — 15, the default threshold.
    const AT_THRESHOLD: &str = r#"Функция РазложитьПосылки(Посылки, Режим)
	Если Посылки = Неопределено Тогда
		Возврат 0;
	КонецЕсли;
	Разложено = 0;
	Для Каждого Посылка Из Посылки Цикл
		Если Посылка.Хрупкая Тогда
			Пока Посылка.НеПроверена() Цикл
				Посылка.Проверить();
			КонецЦикла;
		ИначеЕсли Посылка.Тяжелая И Режим = 1 Тогда
			Разложено = Разложено + 2;
		Иначе
			Попытка
				Разложено = Разложено + 1;
			Исключение
				Продолжить;
			КонецПопытки;
		КонецЕсли;
	КонецЦикла;
	Возврат ?(Разложено > 0, Разложено, -1);
КонецФункции

Процедура БезВетвлений()
	Сообщить("готово");
КонецПроцедуры
"#;

    #[test]
    fn test_simple_function() {
        let code = r#"Функция ВесБрутто(Посылка)
	Возврат Посылка.Вес + Посылка.Упаковка;
КонецФункции
"#;
        check_diagnostics_snapshot_for(code, DiagnosticCode::CognitiveComplexity, expect![[r#""#]]);
    }

    #[test]
    fn test_composition_at_threshold() {
        check_diagnostics_snapshot_for(
            AT_THRESHOLD,
            DiagnosticCode::CognitiveComplexity,
            expect![""],
        );
    }

    #[test]
    fn test_composition_over_threshold() {
        // One more logical operator: 16.
        let code = AT_THRESHOLD.replace("И Режим = 1 Тогда", "И Режим = 1 И Посылка.Длинная Тогда");
        check_diagnostics_snapshot_for(
            &code,
            DiagnosticCode::CognitiveComplexity,
            expect![[r#"
                CognitiveComplexity @ 1:9..1:25
                  message: Функция 'РазложитьПосылки' имеет когнитивную сложность 16 (максимум: 15). Упростите логику или уменьшите вложенность
                  severity: Warning"#]],
        );
    }

    #[test]
    fn test_nested_if_higher_complexity() {
        let code = r#"Процедура ОтметитьХрупкие(Посылки)
	Если Посылки.Количество() > 0 Тогда
		Если Посылки[0].Хрупкая Тогда
			Посылки[0].Отметить();
		КонецЕсли;
	КонецЕсли;
КонецПроцедуры
"#;
        check_with_threshold(
            code,
            2,
            expect![[r#"
            CognitiveComplexity @ 1:11..1:26
              message: Процедура 'ОтметитьХрупкие' имеет когнитивную сложность 3 (максимум: 2). Упростите логику или уменьшите вложенность
              severity: Warning"#]],
        );
    }

    #[test]
    fn test_elseif_no_extra_nesting() {
        // Если 1 + three ИначеЕсли/Иначе at 1 each, no nesting penalty: 4 is the threshold.
        let code = r#"Функция Зона(Индекс)
	Если Индекс < 200000 Тогда
		Возврат "Восток";
	ИначеЕсли Индекс < 400000 Тогда
		Возврат "Центр";
	ИначеЕсли Индекс < 600000 Тогда
		Возврат "Юг";
	Иначе
		Возврат "Запад";
	КонецЕсли;
КонецФункции
"#;
        check_with_threshold(code, 4, expect![[r#""#]]);
    }

    #[test]
    fn test_recursion_penalty_self_call() {
        let code = r#"Функция ГлубинаКоробки(Коробка)
	Если Коробка.Вложенная = Неопределено Тогда
		Возврат 1;
	КонецЕсли;
	Возврат 1 + ГлубинаКоробки(Коробка.Вложенная);
КонецФункции
"#;
        check_with_threshold(
            code,
            1,
            expect![[r#"
            CognitiveComplexity @ 1:9..1:23
              message: Функция 'ГлубинаКоробки' имеет когнитивную сложность 2 (максимум: 1). Упростите логику или уменьшите вложенность
              severity: Warning"#]],
        );
    }

    #[test]
    fn test_compute_hir_metrics_cognitive_value() {
        let fixture_text = format!("//- /test.bsl\n{}", AT_THRESHOLD);
        let fixture = Fixture::parse(&fixture_text);
        let file_id = fixture.first_file().unwrap();

        let mut db = RootDatabaseImpl::new();

        let mut file_set = FileSet::default();
        file_set.insert(file_id, VfsPath::new("/test.bsl"));
        let source_root = SourceRoot::new_local(file_set);
        db.set_source_root(SourceRootId(0), source_root);
        db.set_file_source_root(file_id, SourceRootId(0));

        for (fid, file) in &fixture.files {
            db.set_file_text(*fid, &file.content);
        }

        // Rationale: the test-only database is used from one thread; `RootDatabase`
        // is consumed as `Rc<dyn …>` like the production provider.
        #[allow(clippy::arc_with_non_send_sync)]
        let db = Rc::new(db) as Rc<dyn RootDatabase>;
        let module_id = ModuleId::new(file_id);
        let module_bodies = db.module_bodies(module_id);

        let (_, body) = module_bodies.iter_bodies().next().expect("Should have first method body");
        let metrics = hir::metrics::compute_hir_metrics(body);

        assert_eq!(metrics.cognitive, 15, "РазложитьПосылки should have cognitive 15");
    }
}
