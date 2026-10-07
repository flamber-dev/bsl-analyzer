use crate::define_metadata;
use crate::metadata::*;
use crate::{Diagnostic, DiagnosticCode, DiagnosticsConfig, DiagnosticsContext};

pub const METADATA: DiagnosticMetadata = define_metadata! {
    diagnostic_type: DiagnosticType::CodeSmell,
    severity: DiagnosticSeverityLevel::Minor,
    scope: DiagnosticScope::Bsl,
    modules: &[],
    minutes_to_fix: 5,
    activated_by_default: true,
    compatibility_mode: DiagnosticCompatibilityMode::Undefined,
    tags: &[MetadataTag::Standard, MetadataTag::Sql, MetadataTag::Performance],
    can_locate_on_project: false,
    extra_min_for_complexity: 0.0,
    lsp_severity_override: "",
    clean_code_attribute: CleanCodeAttribute::Adaptable,
};

pub(crate) fn dispatch(
    config: &DiagnosticsConfig,
    diag: &sdbl_hir::SdblDiagnostic,
    mapper: &crate::sdbl_utils::SdblPositionMapper,
    query_text: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let sdbl_hir::SdblDiagnostic::UnionWithoutAll { range } = diag {
        crate::sdbl_utils::dispatch_simple(config, DiagnosticCode::UnionAll, "Использование ключевого слова ОБЪЕДИНИТЬ без ВСЕ приводит к излишней обработке для удаления дубликатов. Используйте ОБЪЕДИНИТЬ ВСЕ", *range, mapper, query_text, diagnostics);
    }
}

pub fn check(ctx: &DiagnosticsContext) -> Vec<Diagnostic> {
    crate::sdbl_utils::collect_sdbl_via_dispatch(ctx, DiagnosticCode::UnionAll, dispatch)
}

#[cfg(test)]
mod tests {
    use crate::test_utils::check_diagnostics_snapshot_for;
    use crate::DiagnosticCode;
    use expect_test::expect;

    fn check(code: &str, expected: expect_test::Expect) {
        check_diagnostics_snapshot_for(code, DiagnosticCode::UnionAll, expected);
    }

    #[test]
    fn test_union_all_and_plain_union_in_one_batch() {
        // Two independent queries in one module: each is analysed on its own.
        let code = r#"Запрос = Новый Запрос(
	"ВЫБРАТЬ РАЗРЕШЕННЫЕ ПЕРВЫЕ 10
	|	Рейсы.Номер КАК Номер,
	|	""Прибытие"" КАК Направление,
	|	ЕСТЬNULL(Рейсы.Перрон, """") КАК Перрон
	|ИЗ
	|	Справочник.РейсыПрибытия КАК Рейсы
	|
	|ОБЪЕДИНИТЬ ВСЕ
	|
	|ВЫБРАТЬ ПЕРВЫЕ 10
	|	Отправления.Номер,
	|	""Отправление"",
	|	ЕСТЬNULL(Отправления.Перрон, """")
	|ИЗ
	|	Справочник.РейсыОтправления КАК Отправления
	|
	|ОБЪЕДИНИТЬ
	|
	|ВЫБРАТЬ
	|	Транзит.Номер,
	|	""Транзит"",
	|	Транзит.Перрон
	|ИЗ
	|	Справочник.ТранзитныеРейсы КАК Транзит");

Табло = Новый Запрос(
	"ВЫБРАТЬ Перроны.Номер
	|ИЗ Справочник.Перроны КАК Перроны
	|ОБЪЕДИНИТЬ
	|ВЫБРАТЬ Пути.Номер
	|ИЗ Справочник.ЗапасныеПути КАК Пути");
"#;
        check(
            code,
            expect![[r#"
                UnionAll @ 18:3..18:13
                  message: Использование ключевого слова ОБЪЕДИНИТЬ без ВСЕ приводит к излишней обработке для удаления дубликатов. Используйте ОБЪЕДИНИТЬ ВСЕ
                  severity: Information
                UnionAll @ 30:3..30:13
                  message: Использование ключевого слова ОБЪЕДИНИТЬ без ВСЕ приводит к излишней обработке для удаления дубликатов. Используйте ОБЪЕДИНИТЬ ВСЕ
                  severity: Information"#]],
        );
    }

    #[test]
    fn test_simple_russian() {
        let code = r#"Процедура Табло()
	Текст = "ВЫБРАТЬ Номер ИЗ Прибытие ОБЪЕДИНИТЬ ВЫБРАТЬ Номер ИЗ Отправление";
КонецПроцедуры
"#;
        check(
            code,
            expect![[r#"
            UnionAll @ 2:37..2:47
              message: Использование ключевого слова ОБЪЕДИНИТЬ без ВСЕ приводит к излишней обработке для удаления дубликатов. Используйте ОБЪЕДИНИТЬ ВСЕ
              severity: Information"#]],
        );
    }

    #[test]
    fn test_simple_english() {
        let code = r#"Procedure Board()
	Text = "SELECT Number FROM Arrivals UNION SELECT Number FROM Departures";
EndProcedure
"#;
        check(
            code,
            expect![[r#"
            UnionAll @ 2:38..2:43
              message: Использование ключевого слова ОБЪЕДИНИТЬ без ВСЕ приводит к излишней обработке для удаления дубликатов. Используйте ОБЪЕДИНИТЬ ВСЕ
              severity: Information"#]],
        );
    }

    #[test]
    fn test_no_false_positives_union_all() {
        let code = r#"Процедура Табло()
	Текст = "ВЫБРАТЬ Номер ИЗ Прибытие ОБЪЕДИНИТЬ ВСЕ ВЫБРАТЬ Номер ИЗ Отправление";
	TextEn = "SELECT Number FROM Arrivals UNION ALL SELECT Number FROM Departures";
КонецПроцедуры
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_multiple_unions() {
        let code = r#"Процедура Табло()
	Текст = "SELECT N FROM A UNION SELECT N FROM B UNION SELECT N FROM C";
КонецПроцедуры
"#;
        check(
            code,
            expect![[r#"
            UnionAll @ 2:27..2:32
              message: Использование ключевого слова ОБЪЕДИНИТЬ без ВСЕ приводит к излишней обработке для удаления дубликатов. Используйте ОБЪЕДИНИТЬ ВСЕ
              severity: Information
            UnionAll @ 2:49..2:54
              message: Использование ключевого слова ОБЪЕДИНИТЬ без ВСЕ приводит к излишней обработке для удаления дубликатов. Используйте ОБЪЕДИНИТЬ ВСЕ
              severity: Information"#]],
        );
    }

    #[test]
    fn test_mixed_union_and_union_all() {
        let code = r#"Процедура Табло()
	Текст = "SELECT N FROM A UNION SELECT N FROM B UNION ALL SELECT N FROM C";
КонецПроцедуры
"#;
        check(
            code,
            expect![[r#"
            UnionAll @ 2:27..2:32
              message: Использование ключевого слова ОБЪЕДИНИТЬ без ВСЕ приводит к излишней обработке для удаления дубликатов. Используйте ОБЪЕДИНИТЬ ВСЕ
              severity: Information"#]],
        );
    }

    #[test]
    fn test_multiline_query() {
        let code = r#"Процедура Табло()
	Текст = "ВЫБРАТЬ Номер
	|ИЗ Прибытие
	|ОБЪЕДИНИТЬ
	|ВЫБРАТЬ Номер
	|ИЗ Отправление";
КонецПроцедуры
"#;
        check(
            code,
            expect![[r#"
            UnionAll @ 4:3..4:13
              message: Использование ключевого слова ОБЪЕДИНИТЬ без ВСЕ приводит к излишней обработке для удаления дубликатов. Используйте ОБЪЕДИНИТЬ ВСЕ
              severity: Information"#]],
        );
    }
}
