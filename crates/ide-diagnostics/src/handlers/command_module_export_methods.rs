use crate::define_metadata;
use crate::metadata::*;
use crate::{Diagnostic, DiagnosticCode, DiagnosticsContext};
use ide_db::TextRange;

pub const METADATA: DiagnosticMetadata = define_metadata! {
    diagnostic_type: DiagnosticType::CodeSmell,
    severity: DiagnosticSeverityLevel::Info,
    scope: DiagnosticScope::Bsl,
    modules: &[bsl_metadata::ModuleType::CommandModule],
    minutes_to_fix: 1,
    activated_by_default: true,
    compatibility_mode: DiagnosticCompatibilityMode::Undefined,
    tags: &[MetadataTag::Standard, MetadataTag::Clumsy],
    can_locate_on_project: false,
    extra_min_for_complexity: 0.0,
    lsp_severity_override: "",
};

pub fn check(ctx: &DiagnosticsContext) -> Vec<Diagnostic> {
    let code = DiagnosticCode::CommandModuleExportMethods;

    if ctx.is_disabled_with_metadata(code) {
        return Vec::new();
    }

    let metadata = ctx.module_metadata();

    if metadata.module_type != bsl_metadata::ModuleType::CommandModule {
        return Vec::new();
    }

    let item_tree = ctx.item_tree();
    let mut diagnostics = Vec::new();

    for (_, proc) in item_tree.procedures() {
        if proc.is_export {
            diagnostics.push(make_diagnostic(proc.name_range, code, ctx));
        }
    }

    for (_, func) in item_tree.functions() {
        if func.is_export {
            diagnostics.push(make_diagnostic(func.name_range, code, ctx));
        }
    }

    diagnostics
}

fn make_diagnostic(range: TextRange, code: DiagnosticCode, ctx: &DiagnosticsContext) -> Diagnostic {
    Diagnostic {
        code,
        message: "Экспортные методы в модулях команд не имеют смысла".to_string(),
        severity: ctx.severity(code),
        range,
        tags: ctx.tags(code),
        fixes: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::format_diags;
    use crate::DiagnosticsConfig;
    use expect_test::expect;
    use ide_db::base_db::{SourceDatabase, SourceRoot, SourceRootId};
    use ide_db::RootDatabaseImpl;
    use std::rc::Rc;
    use vfs::{FileId, FileSet, VfsPath};

    const OBJECT_COMMAND: &str =
        "Documents/ПриходнаяНакладная/Commands/ПечатьЭтикеток/Ext/CommandModule.bsl";
    const COMMON_COMMAND: &str = "CommonCommands/ОбновитьОстатки/Ext/CommandModule.bsl";
    const REGULAR_MODULE: &str = "/СкладскойУчет.bsl";

    fn check_at(path: &str, code: &str) -> Vec<Diagnostic> {
        let mut db = RootDatabaseImpl::new();
        let file_id = FileId::from_raw(1);

        let mut file_set = FileSet::default();
        file_set.insert(file_id, VfsPath::new(path));
        let source_root = SourceRoot::new_local(file_set);
        db.set_source_root(SourceRootId(0), source_root);
        db.set_file_source_root(file_id, SourceRootId(0));

        db.set_file_text(file_id, code);

        let config = Rc::new(DiagnosticsConfig::default());
        let provider = ide_db::SalsaProvider::new(&db, None);
        let ctx = crate::DiagnosticsContext::new(&config, file_id, &provider);

        check(&ctx)
    }

    const EXPORTED: &str = r#"&НаКлиенте
Процедура ОбработкаКоманды(Накладная, ПараметрыВыполнения) Экспорт
	НапечататьЭтикетки(Накладная);
КонецПроцедуры

&НаСервере
Функция ЧислоЭтикеток(Накладная) Экспорт
	Возврат Накладная.Товары.Количество();
КонецФункции

&НаКлиенте
Процедура НапечататьЭтикетки(Накладная)
КонецПроцедуры
"#;

    #[test]
    fn test_exported_procedure_and_function_detected() {
        let diagnostics = check_at(OBJECT_COMMAND, EXPORTED);
        expect![[r#"
            CommandModuleExportMethods @ 2:11..2:27
              message: Экспортные методы в модулях команд не имеют смысла
              severity: Hint
            CommandModuleExportMethods @ 7:9..7:22
              message: Экспортные методы в модулях команд не имеют смысла
              severity: Hint"#]]
        .assert_eq(&format_diags(EXPORTED, &diagnostics));
    }

    #[test]
    fn test_non_exported_ignored() {
        let code = r#"&НаКлиенте
Процедура ОбработкаКоманды(Накладная, ПараметрыВыполнения)
	НапечататьЭтикетки(Накладная);
КонецПроцедуры

&НаСервере
Функция ЧислоЭтикеток(Накладная)
	Возврат Накладная.Товары.Количество();
КонецФункции
"#;
        let diagnostics = check_at(OBJECT_COMMAND, code);
        expect![[r#""#]].assert_eq(&format_diags(code, &diagnostics));
    }

    #[test]
    fn test_common_command_module() {
        let code = r#"&НаКлиенте
Procedure CommandProcessing(CommandParameter, ExecutionParameters) Export
EndProcedure
"#;
        let diagnostics = check_at(COMMON_COMMAND, code);
        expect![[r#"
            CommandModuleExportMethods @ 2:11..2:28
              message: Экспортные методы в модулях команд не имеют смысла
              severity: Hint"#]]
        .assert_eq(&format_diags(code, &diagnostics));
    }

    #[test]
    fn test_regular_module_not_checked() {
        let diagnostics = check_at(REGULAR_MODULE, EXPORTED);
        expect![[r#""#]].assert_eq(&format_diags(EXPORTED, &diagnostics));
    }
}
