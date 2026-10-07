use crate::define_metadata;
use crate::metadata::*;
use crate::{Diagnostic, DiagnosticCode, DiagnosticsContext};
use ide_db::TextRange;

pub const METADATA: DiagnosticMetadata = define_metadata! {
    diagnostic_type: DiagnosticType::CodeSmell,
    severity: DiagnosticSeverityLevel::Major,
    scope: DiagnosticScope::Bsl,
    modules: &[bsl_metadata::ModuleType::FormModule, bsl_metadata::ModuleType::CommandModule],
    minutes_to_fix: 1,
    activated_by_default: true,
    compatibility_mode: DiagnosticCompatibilityMode::Undefined,
    tags: &[MetadataTag::Standard, MetadataTag::Unpredictable],
    can_locate_on_project: false,
    extra_min_for_complexity: 0.0,
    lsp_severity_override: "",
};

pub fn check(ctx: &DiagnosticsContext) -> Vec<Diagnostic> {
    let code = DiagnosticCode::CompilationDirectiveLost;

    if ctx.is_disabled_with_metadata(code) {
        return Vec::new();
    }

    let metadata = ctx.module_metadata();

    if !matches!(
        metadata.module_type,
        bsl_metadata::ModuleType::FormModule | bsl_metadata::ModuleType::CommandModule
    ) {
        return Vec::new();
    }
    // An ordinary form runs only in the thick client; a directive there neither is
    // required nor moves the method.
    if metadata.is_ordinary_form_module() {
        return Vec::new();
    }

    let item_tree = ctx.item_tree();
    let mut diagnostics = Vec::new();

    for (_, proc) in item_tree.procedures() {
        if proc.annotations.is_empty() {
            diagnostics.push(make_diagnostic(&proc.name, proc.name_range, code, ctx));
        }
    }

    for (_, func) in item_tree.functions() {
        if func.annotations.is_empty() {
            diagnostics.push(make_diagnostic(&func.name, func.name_range, code, ctx));
        }
    }

    diagnostics
}

fn make_diagnostic(
    name: &hir::Name,
    range: TextRange,
    code: DiagnosticCode,
    ctx: &DiagnosticsContext,
) -> Diagnostic {
    Diagnostic {
        code,
        message: format!(
            "Пропущена директива компиляции для '{}'. \
             В модулях форм и команд требуется указывать \
             &НаСервере, &НаКлиенте и т.д.",
            name
        ),
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

    const FORM_MODULE: &str = "Catalogs/Теплицы/Forms/ФормаЭлемента/Ext/Form/Module.bsl";
    const COMMAND_MODULE: &str = "CommonCommands/ЗапуститьПолив/Ext/CommandModule.bsl";
    const REGULAR_MODULE: &str = "/ОбщийМодульПолива.bsl";

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

    #[test]
    fn test_with_directive() {
        let code = r#"&НаСервере
Процедура ЗаписатьГрафик()
КонецПроцедуры

&НаКлиенте
Функция ЗаголовокОкна()
	Возврат "Полив";
КонецФункции

&НаСервереБезКонтекста
Функция НормаНаДень(Культура)
	Возврат 0;
КонецФункции
"#;
        let diagnostics = check_at(FORM_MODULE, code);
        expect![[r#""#]].assert_eq(&format_diags(code, &diagnostics));
    }

    #[test]
    fn test_without_directive() {
        let code = r#"&НаСервере
Процедура ЗаписатьГрафик()
КонецПроцедуры

Функция ЗаголовокОкна()
	Возврат "Полив";
КонецФункции
"#;
        let diagnostics = check_at(FORM_MODULE, code);
        expect![[r#"
            CompilationDirectiveLost @ 5:9..5:22
              message: Пропущена директива компиляции для 'ЗаголовокОкна'. В модулях форм и команд требуется указывать &НаСервере, &НаКлиенте и т.д.
              severity: Warning"#]].assert_eq(&format_diags(code, &diagnostics));
    }

    #[test]
    fn test_multiple_missing() {
        let code = r#"Функция ДатаСледующегоПолива()
	Возврат ТекущаяДата();
КонецФункции

&НаКлиенте
Процедура ОбновитьИндикатор()
КонецПроцедуры

Процедура СброситьГрафик()
КонецПроцедуры

Процедура ПоказатьИсторию()
КонецПроцедуры
"#;
        let diagnostics = check_at(FORM_MODULE, code);
        expect![[r#"
            CompilationDirectiveLost @ 1:9..1:29
              message: Пропущена директива компиляции для 'ДатаСледующегоПолива'. В модулях форм и команд требуется указывать &НаСервере, &НаКлиенте и т.д.
              severity: Warning
            CompilationDirectiveLost @ 9:11..9:25
              message: Пропущена директива компиляции для 'СброситьГрафик'. В модулях форм и команд требуется указывать &НаСервере, &НаКлиенте и т.д.
              severity: Warning
            CompilationDirectiveLost @ 12:11..12:26
              message: Пропущена директива компиляции для 'ПоказатьИсторию'. В модулях форм и команд требуется указывать &НаСервере, &НаКлиенте и т.д.
              severity: Warning"#]].assert_eq(&format_diags(code, &diagnostics));
    }

    #[test]
    fn test_english_keywords() {
        let code = r#"&AtClient
Procedure RefreshGauge()
EndProcedure

Procedure ResetSchedule()
EndProcedure
"#;
        let diagnostics = check_at(FORM_MODULE, code);
        expect![[r#"
            CompilationDirectiveLost @ 5:11..5:24
              message: Пропущена директива компиляции для 'ResetSchedule'. В модулях форм и команд требуется указывать &НаСервере, &НаКлиенте и т.д.
              severity: Warning"#]].assert_eq(&format_diags(code, &diagnostics));
    }

    #[test]
    fn test_command_module() {
        let code = r#"&НаКлиенте
Процедура ОбработкаКоманды(ПараметрКоманды, ПараметрыВыполненияКоманды)
	ЗапуститьНаСервере();
КонецПроцедуры

Процедура ЗапуститьНаСервере()
КонецПроцедуры
"#;
        let diagnostics = check_at(COMMAND_MODULE, code);
        expect![[r#"
            CompilationDirectiveLost @ 6:11..6:29
              message: Пропущена директива компиляции для 'ЗапуститьНаСервере'. В модулях форм и команд требуется указывать &НаСервере, &НаКлиенте и т.д.
              severity: Warning"#]].assert_eq(&format_diags(code, &diagnostics));
    }

    #[test]
    fn test_regular_module_not_checked() {
        let code = r#"Процедура ЗаписатьГрафик()
КонецПроцедуры

Функция ЗаголовокОкна()
	Возврат "Полив";
КонецФункции
"#;
        let diagnostics = check_at(REGULAR_MODULE, code);
        expect![[r#""#]].assert_eq(&format_diags(code, &diagnostics));
    }
}
