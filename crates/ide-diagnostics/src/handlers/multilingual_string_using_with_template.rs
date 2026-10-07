use crate::define_metadata;
use crate::metadata::*;
use crate::utils::nstr::{
    extract_language_keys, get_assigned_variable_name, has_template_in_parents, is_nstr_call,
    is_variable_used_in_template, NstrConfig,
};
use crate::BodyContext;
use crate::{sdbl_utils, Diagnostic, DiagnosticCode};
use hir::LocalRange;
use syntax::SyntaxKind;

pub const METADATA: DiagnosticMetadata = define_metadata! {
    diagnostic_type: DiagnosticType::Error,
    severity: DiagnosticSeverityLevel::Major,
    scope: DiagnosticScope::Bsl,
    modules: &[],
    minutes_to_fix: 2,
    activated_by_default: true,
    compatibility_mode: DiagnosticCompatibilityMode::Undefined,
    tags: &[MetadataTag::Error, MetadataTag::Localize],
    can_locate_on_project: false,
    extra_min_for_complexity: 0.0,
    lsp_severity_override: "",
};

pub fn check_body(ctx: &BodyContext, acc: &mut Vec<Diagnostic<LocalRange>>) {
    let _span = tracing::debug_span!("MultilingualStringUsingWithTemplate::check").entered();

    let code = DiagnosticCode::MultilingualStringUsingWithTemplate;

    if ctx.is_disabled_with_metadata(code) {
        return;
    }

    let config = NstrConfig::from_context(ctx, code);

    let mut diagnostics = Vec::new();

    for tok in ctx.tokens() {
        if tok.kind() != SyntaxKind::IDENT || !is_nstr_call(tok.text()) {
            continue;
        }

        let call_expr = match tok
            .parent()
            .and_then(|p| p.ancestors().find(|n| n.kind() == SyntaxKind::CALL_EXPR))
        {
            Some(ce) => ce,
            None => continue,
        };

        let in_template = has_template_in_parents(&call_expr);
        let used_in_template = get_assigned_variable_name(&call_expr)
            .map(|var| is_variable_used_in_template(&var, &call_expr))
            .unwrap_or(false);

        if !in_template && !used_in_template {
            continue;
        }

        let arg_list = call_expr.children().find(|n| n.kind() == SyntaxKind::ARG_LIST);
        let arg_list = match arg_list {
            Some(al) => al,
            None => {
                diagnostics.push(Diagnostic {
                    code,
                    message: format!(
                        "Добавьте строки для языков: [{}]",
                        config.declared_languages.iter().cloned().collect::<Vec<_>>().join(", ")
                    ),
                    severity: ctx.severity(code),
                    range: LocalRange::of_detached_node(call_expr.text_range()),
                    tags: ctx.tags(code),
                    fixes: vec![],
                });
                continue;
            }
        };

        let first_arg = arg_list.children().find(|n| n.kind() == SyntaxKind::EXPR);
        let first_arg = match first_arg {
            Some(a) => a,
            None => {
                diagnostics.push(Diagnostic {
                    code,
                    message: format!(
                        "Добавьте строки для языков: [{}]",
                        config.declared_languages.iter().cloned().collect::<Vec<_>>().join(", ")
                    ),
                    severity: ctx.severity(code),
                    range: LocalRange::of_detached_node(call_expr.text_range()),
                    tags: ctx.tags(code),
                    fixes: vec![],
                });
                continue;
            }
        };

        let literal = first_arg.descendants().find(|n| n.kind() == SyntaxKind::LITERAL);
        let literal = match literal {
            Some(l) => l,
            None => continue,
        };

        let string_content = match sdbl_utils::extract_string_content(&literal) {
            Some(s) => s,
            None => continue,
        };

        let found_languages = extract_language_keys(&string_content);

        let missing: Vec<&String> = config
            .declared_languages
            .iter()
            .filter(|lang| !found_languages.contains(*lang))
            .collect();

        if !missing.is_empty() {
            let missing_str = missing.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ");

            diagnostics.push(Diagnostic {
                code,
                message: format!("Добавьте строки для языков: [{}]", missing_str),
                severity: ctx.severity(code),
                range: LocalRange::of_detached_node(call_expr.text_range()),
                tags: ctx.tags(code),
                fixes: vec![],
            });
        }
    }

    tracing::debug!(
        count = diagnostics.len(),
        "MultilingualStringUsingWithTemplate diagnostics found"
    );

    acc.extend(diagnostics);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{check_body_diagnostic_with_config, format_diags};
    use crate::{DiagnosticCode, DiagnosticsConfig};
    use expect_test::expect;

    fn declared(languages: Option<&str>) -> DiagnosticsConfig {
        let mut config = DiagnosticsConfig::default();
        if let Some(languages) = languages {
            config.parameters.insert(
                DiagnosticCode::MultilingualStringUsingWithTemplate,
                serde_json::json!({ "declaredLanguages": languages }),
            );
        }
        config
    }

    fn snapshot(code: &str, config: DiagnosticsConfig, expected: expect_test::Expect) {
        let diagnostics = check_body_diagnostic_with_config(code, config, check_body);
        expected.assert_eq(&format_diags(code, &diagnostics));
    }

    const MODULE: &str = r#"Процедура СообщитьОЗаказе(Заказ)
	Подпись = НСтр("ru = 'Пекарня у дома';
		|en = 'Corner bakery'");
	Поле = СтрШаблон("%1 из %2", Заказ.Номер, Заказ.Дата);
	Пусто = НСтр();
	Произвольно = НСтр("просто текст без языка");
	Сообщить(НСтр("ru = 'Готово'"));

	Прямо = СтрШаблон(НСтр("ru = 'Заказ %1 испечён'"), Заказ.Номер);
	Прямо = СтрШаблон(НСтр("en = 'Order %1 is baked'"), Заказ.Номер);

	ЧерезПеременную = НСтр("ru = 'Остаток %1 шт.'");
	Сообщить(СтрШаблон(ЧерезПеременную, Заказ.Остаток));

	ЧерезПеременнуюEn = НСтр("en = 'Left %1 pcs.'");
	Сообщить(СтрШаблон(ЧерезПеременнуюEn, Заказ.Остаток));

	Формат = НСтр("ru = 'ЧДЦ=''2'''");
	Курьер.Отправить(НСтр("ru = 'Адрес='"), Заказ.Адрес);
КонецПроцедуры
"#;

    #[test]
    fn test_only_ru() {
        snapshot(
            MODULE,
            declared(None),
            expect![[r#"
            MultilingualStringUsingWithTemplate @ 10:20..10:52
              message: Добавьте строки для языков: [ru]
              severity: Major
            MultilingualStringUsingWithTemplate @ 15:22..15:49
              message: Добавьте строки для языков: [ru]
              severity: Major"#]],
        );
    }

    #[test]
    fn test_ru_and_en() {
        snapshot(
            MODULE,
            declared(Some("ru,en")),
            expect![[r#"
            MultilingualStringUsingWithTemplate @ 9:20..9:51
              message: Добавьте строки для языков: [en]
              severity: Major
            MultilingualStringUsingWithTemplate @ 10:20..10:52
              message: Добавьте строки для языков: [ru]
              severity: Major
            MultilingualStringUsingWithTemplate @ 12:20..12:49
              message: Добавьте строки для языков: [en]
              severity: Major
            MultilingualStringUsingWithTemplate @ 15:22..15:49
              message: Добавьте строки для языков: [ru]
              severity: Major"#]],
        );
    }

    #[test]
    fn test_no_error_when_all_languages_present() {
        let code = r#"Функция ПодписьКоробки(Коробка)
	Возврат СтрШаблон(НСтр("ru = 'Коробка %1'; en = 'Box %1'"), Коробка.Номер);
КонецФункции
"#;
        snapshot(code, declared(Some("ru,en")), expect![[r#""#]]);
    }

    #[test]
    fn test_all_languages_with_escaped_quotes() {
        let code = r#"Функция ПодписьКоробки(Коробка)
	Возврат СтрШаблон(НСтр("en = ""Baker's box %1""; ru = 'Коробка пекаря %1'"), Коробка.Номер);
КонецФункции
"#;
        snapshot(code, declared(Some("ru,en")), expect![[r#""#]]);
    }

    #[test]
    fn test_empty_nstr_in_template() {
        let code = r#"Функция ПодписьКоробки(Коробка)
	Возврат СтрШаблон(НСтр(), Коробка.Номер);
КонецФункции
"#;
        snapshot(
            code,
            declared(Some("ru,en")),
            expect![[r#"
            MultilingualStringUsingWithTemplate @ 2:20..2:26
              message: Добавьте строки для языков: [en, ru]
              severity: Major"#]],
        );
    }

    #[test]
    fn test_nstr_outside_template_not_detected() {
        let code = r#"Процедура Приветствие()
	Текст = НСтр("ru = 'Добро пожаловать'");
	Сообщить(Текст);
КонецПроцедуры
"#;
        snapshot(code, declared(Some("ru,en")), expect![[r#""#]]);
    }
}
