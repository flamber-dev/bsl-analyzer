use crate::define_metadata;
use crate::metadata::*;
use crate::{BodyContext, Diagnostic, DiagnosticCode};
use hir::LocalRange;
use hir::{Body, BodySourceMap, Expr, ExprId, IdConversion, Name};
use stdx::case::CaseExt;

pub const METADATA: DiagnosticMetadata = define_metadata! {
    diagnostic_type: DiagnosticType::CodeSmell,
    severity: DiagnosticSeverityLevel::Minor,
    scope: DiagnosticScope::All,
    modules: &[],
    minutes_to_fix: 10,
    activated_by_default: true,
    compatibility_mode: DiagnosticCompatibilityMode::Undefined,
    tags: &[MetadataTag::Badpractice, MetadataTag::Brainoverload],
    can_locate_on_project: false,
    extra_min_for_complexity: 0.0,
    lsp_severity_override: "",
};

pub fn check_body(ctx: &BodyContext, acc: &mut Vec<Diagnostic<LocalRange>>) {
    let code = DiagnosticCode::NestedConstructorsInStructureDeclaration;

    if ctx.is_disabled_with_metadata(code) {
        return;
    }

    check_body_exprs(ctx.body(), ctx.source_map(), code, ctx, acc);
}

fn check_body_exprs(
    body: &Body,
    source_map: &BodySourceMap,
    code: DiagnosticCode,
    ctx: &BodyContext,
    diagnostics: &mut Vec<Diagnostic<LocalRange>>,
) {
    for (expr_id, expr) in body.exprs_iter() {
        let Expr::New { type_name, args } = expr else {
            continue;
        };

        if !is_structure_or_fixed_structure(type_name) {
            continue;
        }

        if args.len() <= 1 {
            continue;
        }

        let has_nested_constructor_with_params = args.iter().any(|&arg_id| {
            matches!(
                body.expr(ExprId::from_idx(arg_id)),
                Expr::New { args: nested_args, .. } if !nested_args.is_empty()
            )
        });

        if !has_nested_constructor_with_params {
            continue;
        }

        let Some(range) = source_map.expr_range(expr_id) else {
            continue;
        };

        diagnostics.push(Diagnostic {
            code,
            message: "Не используйте конструкторы с параметрами при объявлении структуры"
                .to_string(),
            severity: ctx.severity(code),
            range,
            tags: ctx.tags(code),
            fixes: vec![],
        });
    }
}

fn is_structure_or_fixed_structure(type_name: &Option<Name>) -> bool {
    let Some(name) = type_name else {
        return false;
    };

    let text = name.as_str().fold_lower();
    matches!(text.as_str(), "структура" | "structure" | "фиксированнаяструктура" | "fixedstructure")
}

#[cfg(test)]
mod tests {
    use crate::test_utils::check_diagnostics_snapshot_for;
    use crate::DiagnosticCode;
    use expect_test::expect;

    fn check(code: &str, expected: expect_test::Expect) {
        check_diagnostics_snapshot_for(
            code,
            DiagnosticCode::NestedConstructorsInStructureDeclaration,
            expected,
        );
    }

    #[test]
    fn test_prepared_values_are_silent() {
        let code = r#"Процедура СобратьПосылку(Адрес)
	Габариты = Новый Структура("Длина, Ширина", 40, 30);
	Получатель = Новый Структура;
	Посылка = Новый Структура("Габариты, Получатель, Вложения, Курьер",
		Габариты,
		Получатель,
		Новый Массив,
		Новый Структура);
	Копия = Новый ФиксированнаяСтруктура(Новый Структура("Индекс", Адрес.Индекс));
	Обертка = Новый Структура("Данные, Пометка", ОбернутьДанные(Новый Структура("Ключ", 1)), Ложь);
КонецПроцедуры
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_nested_constructor_with_params() {
        let code = r#"Посылка = Новый Структура("Габариты, Курьер",
	Новый Структура("Длина, Ширина", 40, 30),
	"Пешком");
"#;
        check(
            code,
            expect![[r#"
            NestedConstructorsInStructureDeclaration @ 1:11..3:11
              message: Не используйте конструкторы с параметрами при объявлении структуры
              severity: Information"#]],
        );
    }

    #[test]
    fn test_fixed_structure_outer() {
        let code = r#"Тариф = Новый ФиксированнаяСтруктура("Зона, Ставки", "Север", Новый Соответствие(Ставки));
"#;
        check(
            code,
            expect![[r#"
            NestedConstructorsInStructureDeclaration @ 1:9..1:90
              message: Не используйте конструкторы с параметрами при объявлении структуры
              severity: Information"#]],
        );
    }

    #[test]
    fn test_english_keywords() {
        let code = r#"Parcel = New Structure("Size, Courier",
	New Structure("Length, Width", 40, 30),
	"OnFoot");
Lists = New Structure("Items, Marks", New Array(), New Array());
"#;
        check(
            code,
            expect![[r#"
            NestedConstructorsInStructureDeclaration @ 1:10..3:11
              message: Не используйте конструкторы с параметрами при объявлении структуры
              severity: Information"#]],
        );
    }

    #[test]
    fn test_each_outer_constructor_reported_once() {
        let code = r#"Маршрут = Новый Структура("Откуда, Куда, Пересадки",
	Новый Структура("Город", "Тверь"),
	Новый Структура("Город", "Псков"),
	Новый Структура);
Схема = Новый Структура("Узел",
	Новый Структура("Связи",
		Новый ФиксированнаяСтруктура(Новый Структура)));
"#;
        check(
            code,
            expect![[r#"
            NestedConstructorsInStructureDeclaration @ 1:11..4:18
              message: Не используйте конструкторы с параметрами при объявлении структуры
              severity: Information
            NestedConstructorsInStructureDeclaration @ 5:9..7:50
              message: Не используйте конструкторы с параметрами при объявлении структуры
              severity: Information
            NestedConstructorsInStructureDeclaration @ 6:2..7:49
              message: Не используйте конструкторы с параметрами при объявлении структуры
              severity: Information"#]],
        );
    }

    #[test]
    fn test_multiline_key_list() {
        let code = r#"Признаки = Новый Структура("Вес,
	|Объем,
	|Хрупкость",
	Новый Структура("Единица", "кг"),
	Новый Структура("Единица", "л"),
	Ложь);
"#;
        check(
            code,
            expect![[r#"
            NestedConstructorsInStructureDeclaration @ 1:12..6:7
              message: Не используйте конструкторы с параметрами при объявлении структуры
              severity: Information"#]],
        );
    }
}
