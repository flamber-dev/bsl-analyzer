use crate::define_metadata;
use crate::metadata::*;
use crate::AnalysisContext;
use crate::{Diagnostic, DiagnosticCode};
use hir::LocalRange;

pub const METADATA: DiagnosticMetadata = define_metadata! {
    diagnostic_type: DiagnosticType::Error,
    severity: DiagnosticSeverityLevel::Major,
    scope: DiagnosticScope::All,
    modules: &[],
    minutes_to_fix: 5,
    activated_by_default: true,
    compatibility_mode: DiagnosticCompatibilityMode::Undefined,
    tags: &[MetadataTag::Standard, MetadataTag::Error],
    can_locate_on_project: false,
    extra_min_for_complexity: 0.0,
    lsp_severity_override: "",
    clean_code_attribute: CleanCodeAttribute::Intentional,
};

pub fn from_hir(
    collection: &str,
    range: LocalRange,
    ctx: &AnalysisContext,
) -> Option<Diagnostic<LocalRange>> {
    let code = DiagnosticCode::DeletingCollectionItem;

    if ctx.is_disabled_with_metadata(code) {
        return None;
    }

    Some(Diagnostic {
        code,
        message: format!(
            "Удаление элемента из коллекции '{}' во время итерации по ней может \
             привести к пропуску элементов или ошибкам. Используйте обратный цикл \
             по индексу или соберите элементы для удаления отдельно",
            collection
        ),
        severity: ctx.severity(code),
        range,
        tags: ctx.tags(code),
        fixes: vec![],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{check_hir_diagnostic, format_diags};
    use expect_test::expect;

    fn check(code: &str, expected: expect_test::Expect) {
        let diagnostics: Vec<_> = check_hir_diagnostic(code)
            .into_iter()
            .filter(|d| d.code == DiagnosticCode::DeletingCollectionItem)
            .collect();
        expected.assert_eq(&format_diags(code, &diagnostics));
    }

    #[test]
    fn test_simple_deletion() {
        let code = r#"Процедура ОчиститьКорзину(Корзина)
	Для Каждого Позиция Из Корзина Цикл
		Корзина.Удалить(Позиция);
	КонецЦикла;
КонецПроцедуры
"#;
        check(
            code,
            expect![[r#"
            DeletingCollectionItem @ 3:3..3:27
              message: Удаление элемента из коллекции 'Корзина' во время итерации по ней может привести к пропуску элементов или ошибкам. Используйте обратный цикл по индексу или соберите элементы для удаления отдельно
              severity: Major"#]],
        );
    }

    #[test]
    fn test_different_collection_ok() {
        let code = r#"Процедура ОчиститьКорзину(Корзина, Отложенные)
	Для Каждого Позиция Из Корзина Цикл
		Отложенные.Удалить(Позиция);
	КонецЦикла;
КонецПроцедуры
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_global_delete_ok() {
        let code = r#"Процедура ОчиститьКорзину(Корзина)
	Для Каждого Позиция Из Корзина Цикл
		Удалить(Позиция);
	КонецЦикла;
КонецПроцедуры
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_english_keywords() {
        let code = r#"Procedure ClearCart(Cart)
	FOR EACH Item IN Cart DO
		Cart.Delete(Item);
	ENDDO;
EndProcedure
"#;
        check(
            code,
            expect![[r#"
            DeletingCollectionItem @ 3:3..3:20
              message: Удаление элемента из коллекции 'Cart' во время итерации по ней может привести к пропуску элементов или ошибкам. Используйте обратный цикл по индексу или соберите элементы для удаления отдельно
              severity: Major"#]],
        );
    }

    #[test]
    fn test_case_insensitive() {
        let code = r#"Procedure ClearCart()
	For Each Item In Shop().Carts.Active() Do
		SHOP().carts.ACTIVE().delete(Item + 1);
	EndDo;
EndProcedure
"#;
        check(
            code,
            expect![[r#"
            DeletingCollectionItem @ 3:3..3:41
              message: Удаление элемента из коллекции 'Shop().Carts.Active()' во время итерации по ней может привести к пропуску элементов или ошибкам. Используйте обратный цикл по индексу или соберите элементы для удаления отдельно
              severity: Major"#]],
        );
    }

    #[test]
    fn test_good_different_collection_nested_field() {
        let code = r#"Процедура ОчиститьКорзину(Корзина, Архив)
	Для Каждого Позиция Из Корзина Цикл
		Если Позиция.Количество = 0 Тогда
			Архив.Удалить(Позиция);
		КонецЕсли;
	КонецЦикла;
КонецПроцедуры
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_good_global_delete_nested_field() {
        let code = r#"Процедура ОчиститьКорзину(Корзина)
	Для Каждого Позиция Из Корзина Цикл
		Если Позиция.Количество = 0 Тогда
			Удалить(Позиция);
		КонецЕсли;
	КонецЦикла;
КонецПроцедуры
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_error_chained_field_collection() {
        let code = r#"Процедура ОчиститьКорзину(Заказ)
	Для Каждого Позиция Из Заказ.Корзина Цикл
		Если Позиция.Количество = 0 Тогда
			Заказ.Корзина.Удалить(Позиция);
		КонецЕсли;
	КонецЦикла;
КонецПроцедуры
"#;
        check(
            code,
            expect![[r#"
            DeletingCollectionItem @ 4:4..4:34
              message: Удаление элемента из коллекции 'Заказ.Корзина' во время итерации по ней может привести к пропуску элементов или ошибкам. Используйте обратный цикл по индексу или соберите элементы для удаления отдельно
              severity: Major"#]],
        );
    }

    #[test]
    fn test_error_parenthesized_arg() {
        let code = r#"Procedure ClearCart(Cart)
	For Each Item In Cart Do
		Cart.Delete( ( Item ) );
	EndDo;
EndProcedure
"#;
        check(
            code,
            expect![[r#"
            DeletingCollectionItem @ 3:3..3:26
              message: Удаление элемента из коллекции 'Cart' во время итерации по ней может привести к пропуску элементов или ошибкам. Используйте обратный цикл по индексу или соберите элементы для удаления отдельно
              severity: Major"#]],
        );
    }

    #[test]
    fn test_error_expression_arg() {
        let code = r#"Procedure ClearCart(Cart)
	For Each Item In Cart Do
		Cart.Delete(Item - 1);
	EndDo;
EndProcedure
"#;
        check(
            code,
            expect![[r#"
            DeletingCollectionItem @ 3:3..3:24
              message: Удаление элемента из коллекции 'Cart' во время итерации по ней может привести к пропуску элементов или ошибкам. Используйте обратный цикл по индексу или соберите элементы для удаления отдельно
              severity: Major"#]],
        );
    }

    #[test]
    fn test_error_chained_method_calls() {
        let code = r#"Procedure ClearCart(Shop)
	For Each Item In Shop.Carts().Active Do
		Shop.Carts().Active.Delete(Item);
	EndDo;
EndProcedure
"#;
        check(
            code,
            expect![[r#"
            DeletingCollectionItem @ 3:3..3:35
              message: Удаление элемента из коллекции 'Shop.Carts().Active' во время итерации по ней может привести к пропуску элементов или ошибкам. Используйте обратный цикл по индексу или соберите элементы для удаления отдельно
              severity: Major"#]],
        );
    }

    #[test]
    fn test_break_after_delete_simple() {
        let code = r#"Процедура УбратьПервуюПустую(Корзина)
	Для Каждого Позиция Из Корзина Цикл
		Если Позиция.Количество = 0 Тогда
			Корзина.Удалить(Позиция);
			Прервать;
		КонецЕсли;
	КонецЦикла;
КонецПроцедуры
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_break_after_delete_nested_loops() {
        // Deleting from the inner collection and leaving the inner loop at once is safe.
        let code = r#"Процедура СнятьДубли(Заказы, Корзина)
	Для Каждого Заказ Из Заказы Цикл
		Для Каждого Позиция Из Корзина Цикл
			Если Позиция.Заказ = Заказ Тогда
				Корзина.Удалить(Позиция);
				Прервать;
			КонецЕсли;
		КонецЦикла;
	КонецЦикла;
КонецПроцедуры
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_return_after_delete() {
        let code = r#"Процедура УбратьПервуюПустую(Корзина)
	Для Каждого Позиция Из Корзина Цикл
		Если Позиция.Количество = 0 Тогда
			Корзина.Удалить(Позиция);
			Возврат;
		КонецЕсли;
	КонецЦикла;
КонецПроцедуры
"#;
        check(code, expect![[r#""#]]);
    }

    #[test]
    fn test_delete_without_break_still_error() {
        let code = r#"Процедура УбратьПустые(Корзина)
	Для Каждого Позиция Из Корзина Цикл
		Если Позиция.Количество = 0 Тогда
			Корзина.Удалить(Позиция);
		КонецЕсли;
	КонецЦикла;
КонецПроцедуры
"#;
        check(
            code,
            expect![[r#"
            DeletingCollectionItem @ 4:4..4:28
              message: Удаление элемента из коллекции 'Корзина' во время итерации по ней может привести к пропуску элементов или ошибкам. Используйте обратный цикл по индексу или соберите элементы для удаления отдельно
              severity: Major"#]],
        );
    }
}
