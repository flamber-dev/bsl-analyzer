use crate::define_metadata;
use crate::metadata::*;
use crate::{BodyContext, Diagnostic, DiagnosticCode};
use hir::LocalRange;

pub const METADATA: DiagnosticMetadata = define_metadata! {
    diagnostic_type: DiagnosticType::CodeSmell,
    severity: DiagnosticSeverityLevel::Major,
    scope: DiagnosticScope::All,
    modules: &[],
    minutes_to_fix: 30,
    activated_by_default: true,
    compatibility_mode: DiagnosticCompatibilityMode::Undefined,
    tags: &[MetadataTag::Badpractice],
    can_locate_on_project: false,
    extra_min_for_complexity: 0.0,
    lsp_severity_override: "",
    clean_code_attribute: CleanCodeAttribute::Adaptable,
};

const DEFAULT_MAX_METHOD_SIZE: i64 = 200;

pub fn check_body(ctx: &BodyContext, acc: &mut Vec<Diagnostic<LocalRange>>) {
    let code = DiagnosticCode::MethodSize;
    if ctx.is_disabled_with_metadata(code) {
        return;
    }

    let max_method_size = ctx.config_int(code, "maxMethodSize", DEFAULT_MAX_METHOD_SIZE) as u32;
    let (Some(decl), Some(name_range)) = (ctx.decl(), ctx.method_name_range()) else {
        return;
    };
    let metrics = ctx.hir_metrics();
    if metrics.size_lines <= max_method_size {
        return;
    }
    acc.push(Diagnostic {
        code,
        message: format!(
            "Длина метода \"{}\" равна {}, что больше установленного лимита в {} строк",
            decl.name.as_str(),
            metrics.size_lines,
            max_method_size
        ),
        severity: ctx.severity(code),
        range: name_range,
        tags: ctx.tags(code),
        fixes: vec![],
    });
}

#[cfg(test)]
mod tests {
    use crate::test_utils::{
        check_diagnostics_snapshot_for, check_hir_diagnostic_with_config, format_diags,
    };
    use crate::{DiagnosticCode, DiagnosticsConfig};
    use expect_test::expect;

    /// A method whose node spans `size` lines by the documented formula
    /// `S = line(end) − line(start)`: the header (with its annotations), body lines,
    /// the end keyword.
    fn method(header: &str, end: &str, size: usize) -> String {
        let header_lines = header.lines().count();
        assert!(size >= header_lines);
        let body: String =
            (header_lines..size).map(|i| format!("\tОстаток = Остаток - {i};\n")).collect();
        format!("{header}\n{body}{end}\n")
    }

    /// Module of methods around the default threshold 200, plus short ones.
    fn module() -> String {
        [
            "Процедура Заглушка()\n\nКонецПроцедуры\n".to_string(),
            "Функция Ноль() Возврат 0; КонецФункции\n".to_string(),
            method("Процедура СписатьНаПороге()", "КонецПроцедуры", 200),
            method("Процедура СписатьСверхПорога()", "КонецПроцедуры", 201),
            method("&НаСервере\nФункция ОстатокНаПороге()", "КонецФункции", 200),
            method("&НаСервере\nФункция ОстатокСверхПорога()", "КонецФункции", 201),
            "Функция СПараметром(Шаг = 1)\nКонецФункции\n".to_string(),
        ]
        .join("\n")
    }

    fn method_size_config(max: serde_json::Value) -> DiagnosticsConfig {
        let mut config = DiagnosticsConfig::default();
        config
            .parameters
            .insert(DiagnosticCode::MethodSize, serde_json::json!({ "maxMethodSize": max }));
        config
    }

    fn method_size_diagnostics(code: &str, config: DiagnosticsConfig) -> Vec<crate::Diagnostic> {
        use ide_db::base_db::SourceDatabase;

        let (mut db, file_id) = crate::test_utils::create_test_db(code);
        // Fixture::parse normalizes CRLF; restore the exact bytes to exercise file ranges.
        db.set_file_text(file_id, code);
        crate::file_diagnostics(&db, file_id, &config)
            .into_iter()
            .filter(|d| d.code == DiagnosticCode::MethodSize)
            .collect()
    }

    fn method_size_diags(code: &str, max_method_size: i64) -> String {
        let diagnostics = method_size_diagnostics(code, method_size_config(max_method_size.into()));
        format_diags(code, &diagnostics)
    }

    fn assert_method_size_report(
        code: &str,
        config: DiagnosticsConfig,
        name: &str,
        size: u32,
        max: u32,
    ) {
        let diagnostics = method_size_diagnostics(code, config);
        assert_eq!(diagnostics.len(), 1, "{code:?}: {diagnostics:?}");
        let diag = &diagnostics[0];
        assert_eq!(
            diag.message,
            format!(
                "Длина метода \"{name}\" равна {size}, что больше установленного лимита в {max} строк"
            )
        );
        let start = code.find(name).unwrap();
        assert_eq!(u32::from(diag.range.start()) as usize, start);
        assert_eq!(u32::from(diag.range.end()) as usize, start + name.len());
        assert_eq!(diag.severity, crate::Severity::Warning);
        assert!(diag.tags.is_empty());
        assert!(diag.fixes.is_empty());
    }

    #[test]
    fn test_comprehensive() {
        check_diagnostics_snapshot_for(
            &module(),
            DiagnosticCode::MethodSize,
            expect![[r#"
                MethodSize @ 209:11..209:29
                  message: Длина метода "СписатьСверхПорога" равна 201, что больше установленного лимита в 200 строк
                  severity: Warning
                MethodSize @ 615:9..615:27
                  message: Длина метода "ОстатокСверхПорога" равна 201, что больше установленного лимита в 200 строк
                  severity: Warning"#]],
        );
    }

    #[test]
    fn test_configure_threshold_20() {
        let code = module();
        let diagnostics: Vec<_> = check_hir_diagnostic_with_config(
            &code,
            method_size_config(20.into()),
            crate::diagnostics,
        )
        .into_iter()
        .filter(|d| d.code == DiagnosticCode::MethodSize)
        .collect();
        expect![[r#"
            MethodSize @ 7:11..7:26
              message: Длина метода "СписатьНаПороге" равна 200, что больше установленного лимита в 20 строк
              severity: Warning
            MethodSize @ 209:11..209:29
              message: Длина метода "СписатьСверхПорога" равна 201, что больше установленного лимита в 20 строк
              severity: Warning
            MethodSize @ 413:9..413:24
              message: Длина метода "ОстатокНаПороге" равна 200, что больше установленного лимита в 20 строк
              severity: Warning
            MethodSize @ 615:9..615:27
              message: Длина метода "ОстатокСверхПорога" равна 201, что больше установленного лимита в 20 строк
              severity: Warning"#]].assert_eq(&format_diags(&code, &diagnostics));
    }

    #[test]
    fn test_empty_method() {
        let code = "Функция Пусто()\n\n\nКонецФункции\n";
        check_diagnostics_snapshot_for(code, DiagnosticCode::MethodSize, expect![[r#""#]]);
    }

    #[test]
    fn test_one_liner() {
        let code = "Процедура Пауза() КонецПроцедуры\n";
        check_diagnostics_snapshot_for(code, DiagnosticCode::MethodSize, expect![[r#""#]]);
    }

    #[test]
    fn test_three_line_method_exceeds_threshold_1() {
        let code = "Функция Шаг()\n\tВозврат 1;\nКонецФункции\n";
        expect![[r#"
            MethodSize @ 1:9..1:12
              message: Длина метода "Шаг" равна 2, что больше установленного лимита в 1 строк
              severity: Warning"#]]
        .assert_eq(&method_size_diags(code, 1));
    }

    #[test]
    fn test_two_line_method_equals_threshold_1() {
        let code = "Функция Шаг()\nКонецФункции\n";
        expect![[r#""#]].assert_eq(&method_size_diags(code, 1));
    }

    #[test]
    fn method_size_small_spans_and_strict_thresholds() {
        for (start, end) in [
            ("Function", "EndFunction"),
            ("Процедура", "КонецПроцедуры"),
            ("Procedure", "EndProcedure"),
            ("Функция", "КонецФункции"),
        ] {
            for size in 0..=5 {
                let gap = if size == 0 { " ".to_owned() } else { "\n".repeat(size) };
                let code = format!("{start} Узел(){gap}{end}");
                for max in [size as i64, 1, 0] {
                    let config = method_size_config(max.into());
                    if size as i64 > max {
                        assert_method_size_report(&code, config, "Узел", size as u32, max as u32);
                    } else {
                        assert!(
                            method_size_diagnostics(&code, config).is_empty(),
                            "{code:?}/{max}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn method_size_node_range_variations() {
        // Annotations, comments and blank lines inside the node count; lines outside do not.
        for (code, size) in [
            ("Функция Узел()\n\tВозврат 1; // итог\nКонецФункции", 2),
            ("Procedure Узел()\n\n\n    X = 1;\nEndProcedure", 4),
            ("Процедура Узел()\n\t// пояснение\n\tХ = 1;\nКонецПроцедуры", 3),
            ("&НаКлиенте\nПроцедура Узел()\nКонецПроцедуры", 2),
            ("&AtServer\n&Перед(\"Прочее\")\nFunction Узел()\n\n    Return 0;\nEndFunction", 5),
        ] {
            for prefix in ["", "// снаружи\n\n"] {
                for suffix in ["", "\n// после\n\n"] {
                    for newline in ["\n", "\r\n"] {
                        let file = format!("{prefix}{code}{suffix}").replace('\n', newline);
                        assert_method_size_report(
                            &file,
                            method_size_config(1.into()),
                            "Узел",
                            size,
                            1,
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn method_size_default_boundary_and_non_integer_fallback() {
        let at = method("Функция Узел()", "КонецФункции", 200);
        let over = method("Функция Узел()", "КонецФункции", 201);
        for config in [
            DiagnosticsConfig::default(),
            method_size_config(200.into()),
            method_size_config(serde_json::Value::Null),
            method_size_config(serde_json::json!("7")),
            method_size_config(serde_json::json!(2.5)),
            method_size_config(serde_json::json!(false)),
            method_size_config(serde_json::json!(u64::MAX)),
        ] {
            assert!(method_size_diagnostics(&at, config.clone()).is_empty());
            assert_method_size_report(&over, config, "Узел", 201, 200);
        }
    }

    #[test]
    fn method_size_threshold_keeps_i64_to_u32_cast() {
        let code = "Функция Узел()\n\tВозврат 2;\nКонецФункции";
        for (configured, effective) in [
            (i64::MAX, u32::MAX),
            (-1_i64, u32::MAX),
            (4_294_967_297, 1),
            (4_294_967_296, 0),
            (-4_294_967_294, 2),
            (-4_294_967_295, 1),
            (-4_294_967_296, 0),
            (i64::MIN, 0),
        ] {
            let config = method_size_config(configured.into());
            if effective < 2 {
                assert_method_size_report(code, config, "Узел", 2, effective);
            } else {
                assert!(method_size_diagnostics(code, config).is_empty(), "{configured}");
            }
        }
    }

    #[test]
    fn method_size_disabled_keeps_other_diagnostics() {
        let code = "Процедура Узел(Х)\n\tХ = Х;\nКонецПроцедуры";
        let enabled = method_size_config(1.into());
        assert_method_size_report(code, enabled.clone(), "Узел", 2, 1);
        let mut disabled = enabled.clone();
        disabled.disabled.push(DiagnosticCode::MethodSize);
        assert!(method_size_diagnostics(code, disabled.clone()).is_empty());
        let before = check_hir_diagnostic_with_config(code, enabled, crate::diagnostics);
        let after = check_hir_diagnostic_with_config(code, disabled, crate::diagnostics);
        assert!(after.iter().any(|d| d.code == DiagnosticCode::SelfAssign));
        let others: Vec<_> =
            before.into_iter().filter(|d| d.code != DiagnosticCode::MethodSize).collect();
        assert_eq!(others, after);
    }
}
