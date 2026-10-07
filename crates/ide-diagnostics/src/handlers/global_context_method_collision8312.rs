use crate::define_metadata;
use crate::metadata::*;
use crate::AnalysisContext;
use crate::{Diagnostic, DiagnosticCode};
use hir::LocalRange;

pub const METADATA: DiagnosticMetadata = define_metadata! {
    diagnostic_type: DiagnosticType::Error,
    severity: DiagnosticSeverityLevel::Blocker,
    scope: DiagnosticScope::All,
    modules: &[],
    minutes_to_fix: 10,
    activated_by_default: true,
    compatibility_mode: DiagnosticCompatibilityMode::CompatibilityMode8_3_12,
    tags: &[MetadataTag::Error, MetadataTag::Unpredictable],
    can_locate_on_project: false,
    extra_min_for_complexity: 0.0,
    lsp_severity_override: "",
};

pub fn from_hir(
    method_name: &str,
    range: LocalRange,
    ctx: &AnalysisContext,
) -> Option<Diagnostic<LocalRange>> {
    let code = DiagnosticCode::GlobalContextMethodCollision8312;

    if ctx.is_disabled_with_metadata(code) {
        return None;
    }

    Some(Diagnostic {
        code,
        message: format!(
            "Имя метода \"{}\" конфликтует с методом глобального контекста, появившимся в версии платформы 8.3.12",
            method_name
        ),
        severity: ctx.severity(code),
        range,
        tags: ctx.tags(code),
        fixes: vec![],
    })
}

#[cfg(test)]
mod tests {
    use crate::test_utils::*;
    use crate::{DiagnosticCode, Severity};
    use expect_test::expect;

    /// The platform 8.3.12 bitwise methods, RU then EN, as the rule's docs list them.
    const PLATFORM_NAMES: [&str; 20] = [
        "ПобитовыйСдвигВправо",
        "ПобитовыйСдвигВлево",
        "ПобитовоеИсключительноеИли",
        "ПобитовоеИНе",
        "ПобитовоеНе",
        "ПобитовоеИли",
        "ПобитовоеИ",
        "УстановитьБит",
        "ПроверитьПоБитовойМаске",
        "ПроверитьБит",
        "BitwiseShiftRight",
        "BitwiseShiftLeft",
        "BitwiseXor",
        "BitwiseAndNot",
        "BitwiseNot",
        "BitwiseOr",
        "BitwiseAnd",
        "SetBit",
        "CheckByBitMask",
        "CheckBit",
    ];

    fn collisions(code: &str) -> Vec<crate::Diagnostic> {
        check_hir_diagnostic(code)
            .into_iter()
            .filter(|d| d.code == DiagnosticCode::GlobalContextMethodCollision8312)
            .collect()
    }

    #[test]
    fn test_every_platform_name_collides() {
        // Procedures and functions alternate, each with a parameter, so the rule is shown
        // to look at the name alone.
        let code: String = PLATFORM_NAMES
            .iter()
            .enumerate()
            .map(|(i, name)| {
                if i % 2 == 0 {
                    format!("Процедура {name}(Флаги) Экспорт\nКонецПроцедуры\n\n")
                } else {
                    format!("Функция {name}(Флаги)\n\tВозврат Флаги;\nКонецФункции\n\n")
                }
            })
            .collect();
        let diagnostics = collisions(&code);
        let named: Vec<_> = diagnostics
            .iter()
            .map(|d| {
                let start = usize::from(d.range.start());
                let end = usize::from(d.range.end());
                code[start..end].to_string()
            })
            .collect();
        assert_eq!(named, PLATFORM_NAMES, "каждое имя таблицы отмечено ровно по своему имени");
        for (diagnostic, name) in diagnostics.iter().zip(PLATFORM_NAMES) {
            assert_eq!(diagnostic.severity, Severity::Blocker);
            assert_eq!(
                diagnostic.message,
                format!(
                    "Имя метода \"{name}\" конфликтует с методом глобального контекста, появившимся в версии платформы 8.3.12"
                ),
                "сообщение называет имя своего метода"
            );
        }
        expect![[r#"Имя метода "ПобитовыйСдвигВправо" конфликтует с методом глобального контекста, появившимся в версии платформы 8.3.12"#]].assert_eq(&diagnostics[0].message);
    }

    #[test]
    fn test_no_collision() {
        let code = r#"Функция МаскаПрав(Пользователь)
	Возврат Пользователь.Права;
КонецФункции

Процедура СдвинутьОкно(Окно)
КонецПроцедуры
"#;
        expect![[r#""#]].assert_eq(&format_diags(code, &collisions(code)));
    }

    #[test]
    fn test_no_collision_with_prefix_suffix() {
        let code = r#"Функция ЛокальноеПроверитьБит(Флаги)
	Возврат Флаги;
КонецФункции

Процедура УстановитьБитФлага(Флаги)
КонецПроцедуры

Function BitwiseOr2(Flags)
	Return Flags;
EndFunction
"#;
        expect![[r#""#]].assert_eq(&format_diags(code, &collisions(code)));
    }

    #[test]
    fn test_case_insensitive_russian() {
        let code = r#"Процедура побитовоеисключительноеили(Флаги)
КонецПроцедуры
"#;
        expect![[r#"
            GlobalContextMethodCollision8312 @ 1:11..1:37
              message: Имя метода "побитовоеисключительноеили" конфликтует с методом глобального контекста, появившимся в версии платформы 8.3.12
              severity: Blocker"#]].assert_eq(&format_diags(code, &collisions(code)));
    }

    #[test]
    fn test_case_insensitive_english() {
        let code = r#"Procedure BITWISESHIFTLEFT(Flags)
EndProcedure
"#;
        expect![[r#"
            GlobalContextMethodCollision8312 @ 1:11..1:27
              message: Имя метода "BITWISESHIFTLEFT" конфликтует с методом глобального контекста, появившимся в версии платформы 8.3.12
              severity: Blocker"#]].assert_eq(&format_diags(code, &collisions(code)));
    }

    #[test]
    fn test_multiple_collisions() {
        let code = r#"Функция МаскаПрав(Пользователь)
	Возврат Пользователь.Права;
КонецФункции

Функция ПобитовоеНе(Флаги)
	Возврат Флаги;
КонецФункции

Function SetBit(Flags)
	Return Flags;
EndFunction
"#;
        expect![[r#"
            GlobalContextMethodCollision8312 @ 5:9..5:20
              message: Имя метода "ПобитовоеНе" конфликтует с методом глобального контекста, появившимся в версии платформы 8.3.12
              severity: Blocker
            GlobalContextMethodCollision8312 @ 9:10..9:16
              message: Имя метода "SetBit" конфликтует с методом глобального контекста, появившимся в версии платформы 8.3.12
              severity: Blocker"#]].assert_eq(&format_diags(code, &collisions(code)));
    }
}
