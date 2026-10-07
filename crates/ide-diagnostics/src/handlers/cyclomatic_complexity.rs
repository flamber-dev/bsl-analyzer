use crate::define_metadata;
use crate::metadata::*;
use crate::{BodyContext, Diagnostic, DiagnosticCode};
use hir::LocalRange;

pub const METADATA: DiagnosticMetadata = define_metadata! {
    diagnostic_type: DiagnosticType::CodeSmell,
    severity: DiagnosticSeverityLevel::Critical,
    scope: DiagnosticScope::All,
    modules: &[],
    minutes_to_fix: 25,
    activated_by_default: true,
    compatibility_mode: DiagnosticCompatibilityMode::Undefined,
    tags: &[MetadataTag::Brainoverload],
    can_locate_on_project: false,
    extra_min_for_complexity: 1.0,
    lsp_severity_override: "",
};

pub fn check_body(ctx: &BodyContext, acc: &mut Vec<Diagnostic<LocalRange>>) {
    let code = DiagnosticCode::CyclomaticComplexity;
    if ctx.is_disabled_with_metadata(code) {
        return;
    }

    let threshold = ctx.config_int(code, "complexityThreshold", 20) as u32;
    let (Some(decl), Some(complexity), Some(name_range)) =
        (ctx.decl(), ctx.cyclomatic(), ctx.method_name_range())
    else {
        return;
    };
    if complexity <= threshold {
        return;
    }
    let method_type = if decl.is_function { "Функция" } else { "Процедура" };
    acc.push(Diagnostic {
        code,
        message: format!(
            "{} '{}' имеет цикломатическую сложность {} (максимум: {}). \
             Рассмотрите возможность упрощения или разбиения на более мелкие функции",
            method_type,
            decl.name.as_str(),
            complexity,
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

    /// Expected by hand: CFG decisions Если, ИначеЕсли, Для Каждого, Если, Для, Пока,
    /// Попытка, Если — 8, so the CFG metric is 9; plus 2 logical operators and 1 `?()`:
    /// 12 in total.
    const ROUTE: &str = r#"Функция ОценкаМаршрута(Маршрут, Погода, Груз)
	Баллы = 0;
	Если Погода = "Снег" Тогда
		Баллы = Баллы + 3;
	ИначеЕсли Погода = "Дождь" Тогда
		Баллы = Баллы + 2;
	Иначе
		Баллы = Баллы + 1;
	КонецЕсли;
	Для Каждого Участок Из Маршрут.Участки Цикл
		Если Участок.Платный Тогда
			Баллы = Баллы + Участок.Цена;
		КонецЕсли;
	КонецЦикла;
	Для Номер = 1 По Груз.Мест Цикл
		Баллы = Баллы + ?(Груз.Хрупкий, 2, 1);
	КонецЦикла;
	Пока Баллы > 100 Цикл
		Баллы = Баллы / 2;
	КонецЦикла;
	Попытка
		Баллы = Баллы + Маршрут.Надбавка();
	Исключение
		Баллы = Баллы + 5;
	КонецПопытки;
	Если Маршрут.Ночной И Груз.Ценный ИЛИ Погода = "Туман" Тогда
		Баллы = Баллы * 2;
	КонецЕсли;
	Возврат Баллы;
КонецФункции
"#;

    fn check_with_threshold(code: &str, threshold: i64, expected: expect_test::Expect) {
        let mut config = DiagnosticsConfig::default();
        config.parameters.insert(
            DiagnosticCode::CyclomaticComplexity,
            serde_json::json!({ "complexityThreshold": threshold }),
        );
        let diagnostics: Vec<_> =
            check_hir_diagnostic_with_config(code, config, crate::diagnostics)
                .into_iter()
                .filter(|d| d.code == DiagnosticCode::CyclomaticComplexity)
                .collect();
        expected.assert_eq(&format_diags(code, &diagnostics));
    }

    /// A procedure of `ifs` sequential one-way `Если`: CFG complexity `ifs + 1`.
    fn sequential_ifs(ifs: usize) -> String {
        let mut code = String::from("Процедура РазобратьФлаги(Флаги)\n");
        for i in 0..ifs {
            code.push_str(&format!("\tЕсли Флаги[{i}] Тогда\n\t\tСчетчик = {i};\n\tКонецЕсли;\n"));
        }
        code.push_str("КонецПроцедуры\n");
        code
    }

    #[test]
    fn test_simple_function() {
        let code = r#"Функция Остаток(Всего, Занято)
	Возврат Всего - Занято;
КонецФункции
"#;
        check_diagnostics_snapshot_for(
            code,
            DiagnosticCode::CyclomaticComplexity,
            expect![[r#""#]],
        );
    }

    #[test]
    fn test_else_counts() {
        // Если/Иначе is one decision: 2, far below the default threshold.
        let code = r#"Функция Направление(Знак)
	Если Знак > 0 Тогда
		Возврат "вперёд";
	Иначе
		Возврат "назад";
	КонецЕсли;
КонецФункции
"#;
        check_with_threshold(code, 2, expect![[r#""#]]);
    }

    #[test]
    fn test_route_at_threshold() {
        check_with_threshold(ROUTE, 12, expect![[r#""#]]);
    }

    #[test]
    fn test_route_over_threshold() {
        check_with_threshold(
            ROUTE,
            11,
            expect![[r#"
            CyclomaticComplexity @ 1:9..1:23
              message: Функция 'ОценкаМаршрута' имеет цикломатическую сложность 12 (максимум: 11). Рассмотрите возможность упрощения или разбиения на более мелкие функции
              severity: Warning"#]],
        );
    }

    #[test]
    fn test_default_threshold_boundary() {
        check_diagnostics_snapshot_for(
            &sequential_ifs(19),
            DiagnosticCode::CyclomaticComplexity,
            expect![[r#""#]],
        );
    }

    #[test]
    fn test_high_complexity_triggers_diagnostic() {
        check_diagnostics_snapshot_for(
            &sequential_ifs(20),
            DiagnosticCode::CyclomaticComplexity,
            expect![[r#"
                CyclomaticComplexity @ 1:11..1:25
                  message: Процедура 'РазобратьФлаги' имеет цикломатическую сложность 21 (максимум: 20). Рассмотрите возможность упрощения или разбиения на более мелкие функции
                  severity: Warning"#]],
        );
    }

    #[test]
    fn test_calculate_complexity_directly() {
        let fixture_text = format!("//- /test.bsl\n{}", ROUTE);
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

        let cfg = hir::cfg::CfgBuilder::new().build_graph_from_hir(body.body_stmts_typed(), body);
        let complexity = hir::cfg::cyclomatic_complexity(&cfg);
        assert_eq!(complexity, 9, "ОценкаМаршрута CFG-based cyclomatic should be 9");

        let metrics = hir::metrics::compute_hir_metrics(body);
        assert_eq!(metrics.boolean_ops_count, 2, "two logical operators");
        assert_eq!(metrics.ternary_count, 1, "one ternary expression");
        assert_eq!(complexity + metrics.boolean_ops_count + metrics.ternary_count, 12);
    }
}
