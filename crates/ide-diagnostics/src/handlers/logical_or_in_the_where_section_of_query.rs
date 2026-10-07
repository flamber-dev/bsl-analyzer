use crate::define_metadata;
use crate::metadata::*;
use crate::{Diagnostic, DiagnosticCode, DiagnosticsConfig, DiagnosticsContext};

pub const METADATA: DiagnosticMetadata = define_metadata! {
    diagnostic_type: DiagnosticType::CodeSmell,
    severity: DiagnosticSeverityLevel::Major,
    scope: DiagnosticScope::Bsl,
    modules: &[],
    minutes_to_fix: 15,
    activated_by_default: true,
    compatibility_mode: DiagnosticCompatibilityMode::Undefined,
    tags: &[MetadataTag::Sql, MetadataTag::Performance, MetadataTag::Standard],
    can_locate_on_project: false,
    extra_min_for_complexity: 0.0,
    lsp_severity_override: "",
};

pub(crate) fn dispatch(
    config: &DiagnosticsConfig,
    diag: &sdbl_hir::SdblDiagnostic,
    mapper: &crate::sdbl_utils::SdblPositionMapper,
    query_text: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let sdbl_hir::SdblDiagnostic::LogicalOrInWhere { range } = diag {
        crate::sdbl_utils::dispatch_simple(config, DiagnosticCode::LogicalOrInTheWhereSectionOfQuery, "Использование оператора ИЛИ в условии ГДЕ существенно снижает производительность запроса. Рассмотрите возможность переписать с использованием ОБЪЕДИНИТЬ или изменить структуру условий", *range, mapper, query_text, diagnostics);
    }
}

pub fn check(ctx: &DiagnosticsContext) -> Vec<Diagnostic> {
    crate::sdbl_utils::collect_sdbl_via_dispatch(
        ctx,
        DiagnosticCode::LogicalOrInTheWhereSectionOfQuery,
        dispatch,
    )
}

#[cfg(test)]
mod tests {
    use crate::test_utils::check_diagnostics_snapshot_for;
    use crate::DiagnosticCode;
    use expect_test::expect;

    fn check(code: &str, expected: expect_test::Expect) {
        check_diagnostics_snapshot_for(
            code,
            DiagnosticCode::LogicalOrInTheWhereSectionOfQuery,
            expected,
        );
    }

    #[test]
    fn test_and_only_in_where_is_silent() {
        let code = r#"Процедура Отобрать(Запрос)
	Запрос.Текст =
	"ВЫБРАТЬ Книги.Название
	|ИЗ Справочник.Книги КАК Книги
	|ГДЕ
	|	Книги.Автор = &Автор
	|	И Книги.Год > 1900";
КонецПроцедуры
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_multi_case_where_or() {
        // Any ИЛИ in ГДЕ is reported, even over one field or between constants; the
        // current rule has no single-field exception. ИЛИ inside ВЫБОР, in ПО and in
        // УПОРЯДОЧИТЬ ПО is not in ГДЕ.
        let code = r#"Процедура Отобрать(Запрос)
	Запрос.Текст =
	"ВЫБРАТЬ Книги.Название
	|ИЗ Справочник.Книги КАК Книги
	|ГДЕ
	|	Ложь ИЛИ Книги.Год = 1999";

	Запрос.Текст =
	"ВЫБРАТЬ Книги.Название
	|ИЗ Справочник.Книги КАК Книги
	|ГДЕ
	|	Книги.Год = &Год1
	|	ИЛИ Книги.Год = &Год2";

	Запрос.Текст =
	"ВЫБРАТЬ Книги.Название
	|ИЗ Справочник.Книги КАК Книги
	|ГДЕ
	|	Книги.Автор = &Автор
	|	ИЛИ
	|	(Книги.Серия = &Серия ИЛИ Книги.Издатель = &Издатель)";

	Запрос.Текст =
	"ВЫБРАТЬ Ложь КАК Флаг
	|ИЗ Справочник.Книги КАК Книги
	|ГДЕ Книги.Выдана
	|	И Книги.Ссылка В
	|		(ВЫБРАТЬ Брони.Книга
	|		ИЗ РегистрСведений.Брони КАК Брони
	|		ГДЕ Брони.Активна ИЛИ Брони.Продлена)";

	Запрос.Текст =
	"ВЫБРАТЬ ВЫБОР КОГДА Книги.Редкая ИЛИ Книги.Ветхая ТОГДА 1 ИНАЧЕ 0 КОНЕЦ КАК Хранение
	|ИЗ Справочник.Книги КАК Книги
	|	ЛЕВОЕ СОЕДИНЕНИЕ РегистрСведений.Брони КАК Брони
	|	ПО Ложь ИЛИ Книги.Ссылка = Брони.Книга
	|УПОРЯДОЧИТЬ ПО
	|	ВЫБОР КОГДА Книги.Редкая ИЛИ Книги.Ветхая ТОГДА 1 ИНАЧЕ 0 КОНЕЦ";
КонецПроцедуры
"#;
        check(
            code,
            expect![[r#"
            LogicalOrInTheWhereSectionOfQuery @ 6:9..6:12
              message: Использование оператора ИЛИ в условии ГДЕ существенно снижает производительность запроса. Рассмотрите возможность переписать с использованием ОБЪЕДИНИТЬ или изменить структуру условий
              severity: Warning
            LogicalOrInTheWhereSectionOfQuery @ 13:4..13:7
              message: Использование оператора ИЛИ в условии ГДЕ существенно снижает производительность запроса. Рассмотрите возможность переписать с использованием ОБЪЕДИНИТЬ или изменить структуру условий
              severity: Warning
            LogicalOrInTheWhereSectionOfQuery @ 20:4..20:7
              message: Использование оператора ИЛИ в условии ГДЕ существенно снижает производительность запроса. Рассмотрите возможность переписать с использованием ОБЪЕДИНИТЬ или изменить структуру условий
              severity: Warning
            LogicalOrInTheWhereSectionOfQuery @ 21:26..21:29
              message: Использование оператора ИЛИ в условии ГДЕ существенно снижает производительность запроса. Рассмотрите возможность переписать с использованием ОБЪЕДИНИТЬ или изменить структуру условий
              severity: Warning
            LogicalOrInTheWhereSectionOfQuery @ 30:23..30:26
              message: Использование оператора ИЛИ в условии ГДЕ существенно снижает производительность запроса. Рассмотрите возможность переписать с использованием ОБЪЕДИНИТЬ или изменить структуру условий
              severity: Warning"#]],
        );
    }

    #[test]
    fn test_simple_or_in_where() {
        let code = r#"Процедура Отобрать(Запрос)
	Запрос.Текст = "SELECT Title FROM Books WHERE Year = 1999 OR Author = &Author";
КонецПроцедуры
"#;
        check(
            code,
            expect![[r#"
            LogicalOrInTheWhereSectionOfQuery @ 2:60..2:62
              message: Использование оператора ИЛИ в условии ГДЕ существенно снижает производительность запроса. Рассмотрите возможность переписать с использованием ОБЪЕДИНИТЬ или изменить структуру условий
              severity: Warning"#]],
        );
    }

    #[test]
    fn test_russian_or_keyword() {
        let code = r#"Процедура Отобрать(Запрос)
	Запрос.Текст = "ВЫБРАТЬ * ИЗ Книги ГДЕ Год = 1999 ИЛИ Автор = &Автор";
КонецПроцедуры
"#;
        check(
            code,
            expect![[r#"
            LogicalOrInTheWhereSectionOfQuery @ 2:52..2:55
              message: Использование оператора ИЛИ в условии ГДЕ существенно снижает производительность запроса. Рассмотрите возможность переписать с использованием ОБЪЕДИНИТЬ или изменить структуру условий
              severity: Warning"#]],
        );
    }

    #[test]
    fn test_or_in_parentheses() {
        let code = r#"Процедура Отобрать(Запрос)
	Запрос.Текст = "SELECT * FROM Books WHERE Shelf = 3 AND (Year = 1999 OR Year = 2001)";
КонецПроцедуры
"#;
        check(
            code,
            expect![[r#"
            LogicalOrInTheWhereSectionOfQuery @ 2:71..2:73
              message: Использование оператора ИЛИ в условии ГДЕ существенно снижает производительность запроса. Рассмотрите возможность переписать с использованием ОБЪЕДИНИТЬ или изменить структуру условий
              severity: Warning"#]],
        );
    }

    #[test]
    fn test_multiple_or_in_where() {
        let code = r#"Процедура Отобрать(Запрос)
	Запрос.Текст = "SELECT * FROM Books WHERE Shelf = 1 OR Shelf = 2 OR Shelf = 3";
КонецПроцедуры
"#;
        check(
            code,
            expect![[r#"
            LogicalOrInTheWhereSectionOfQuery @ 2:54..2:56
              message: Использование оператора ИЛИ в условии ГДЕ существенно снижает производительность запроса. Рассмотрите возможность переписать с использованием ОБЪЕДИНИТЬ или изменить структуру условий
              severity: Warning
            LogicalOrInTheWhereSectionOfQuery @ 2:67..2:69
              message: Использование оператора ИЛИ в условии ГДЕ существенно снижает производительность запроса. Рассмотрите возможность переписать с использованием ОБЪЕДИНИТЬ или изменить структуру условий
              severity: Warning"#]],
        );
    }

    #[test]
    fn test_nested_subquery() {
        let code = r#"Процедура Отобрать(Запрос)
	Запрос.Текст = "SELECT * FROM Books WHERE Ref IN (SELECT Book FROM Holds WHERE Active OR Extended)";
КонецПроцедуры
"#;
        check(
            code,
            expect![[r#"
            LogicalOrInTheWhereSectionOfQuery @ 2:88..2:90
              message: Использование оператора ИЛИ в условии ГДЕ существенно снижает производительность запроса. Рассмотрите возможность переписать с использованием ОБЪЕДИНИТЬ или изменить структуру условий
              severity: Warning"#]],
        );
    }

    #[test]
    fn test_no_false_positives_case_expression() {
        let code = r#"Процедура Отобрать(Запрос)
	Запрос.Текст = "SELECT CASE WHEN Rare OR Old THEN 1 ELSE 0 END AS Keep FROM Books WHERE Shelf = 1";
КонецПроцедуры
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_no_false_positives_join_on() {
        let code = r#"Процедура Отобрать(Запрос)
	Запрос.Текст = "SELECT * FROM Books AS B LEFT JOIN Holds AS H ON B.Ref = H.Book OR B.Copy = H.Book WHERE B.Shelf = 1";
КонецПроцедуры
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_no_where_clause() {
        let code = r#"Процедура Отобрать(Запрос)
	Запрос.Текст = "SELECT * FROM Books";
КонецПроцедуры
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_sdbl_with_parameters() {
        let code = r#"Процедура Отобрать(Запрос)
	Запрос.Текст = "SELECT * FROM Books WHERE Shelf = &Shelf AND (Author = &Author OR Year = &Year)";
КонецПроцедуры
"#;
        check(
            code,
            expect![[r#"
            LogicalOrInTheWhereSectionOfQuery @ 2:81..2:83
              message: Использование оператора ИЛИ в условии ГДЕ существенно снижает производительность запроса. Рассмотрите возможность переписать с использованием ОБЪЕДИНИТЬ или изменить структуру условий
              severity: Warning"#]],
        );
    }

    #[test]
    fn test_or_in_russian_subquery_where() {
        let code = r#"Процедура Отобрать(Запрос)
	Запрос.Текст =
		"ВЫБРАТЬ Книги.Ссылка КАК Ссылка
		|ИЗ Справочник.Книги КАК Книги
		|ГДЕ Книги.Ссылка В (
		|	ВЫБРАТЬ Выдачи.Книга КАК Книга
		|	ИЗ РегистрСведений.Выдачи КАК Выдачи
		|	ГДЕ Выдачи.Читатель = &Читатель ИЛИ Выдачи.Просрочена
		|)";
КонецПроцедуры
"#;
        check(
            code,
            expect![[r#"
            LogicalOrInTheWhereSectionOfQuery @ 8:37..8:40
              message: Использование оператора ИЛИ в условии ГДЕ существенно снижает производительность запроса. Рассмотрите возможность переписать с использованием ОБЪЕДИНИТЬ или изменить структуру условий
              severity: Warning"#]],
        );
    }

    #[test]
    fn test_or_in_deep_subquery_where() {
        let code = r#"Процедура Отобрать(Запрос)
	Запрос.Текст =
		"SELECT Books.Ref AS Ref
		|FROM Catalog.Books AS Books
		|WHERE Books.Ref IN (
		|	SELECT Loans.Book AS Book
		|	FROM (
		|		SELECT Issued.Book AS Book
		|		FROM InformationRegister.Issued AS Issued
		|		WHERE Issued.Reader = &Reader OR Issued.Overdue
		|	) AS Loans
		|)";
КонецПроцедуры
"#;
        check(
            code,
            expect![[r#"
            LogicalOrInTheWhereSectionOfQuery @ 10:36..10:38
              message: Использование оператора ИЛИ в условии ГДЕ существенно снижает производительность запроса. Рассмотрите возможность переписать с использованием ОБЪЕДИНИТЬ или изменить структуру условий
              severity: Warning"#]],
        );
    }
}
