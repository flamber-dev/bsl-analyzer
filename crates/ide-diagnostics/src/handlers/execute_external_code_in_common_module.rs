use crate::define_metadata;
use crate::metadata::*;
use crate::{common_module_helpers, Diagnostic, DiagnosticCode, DiagnosticsContext};
use syntax::SyntaxKind;

pub const METADATA: DiagnosticMetadata = define_metadata! {
    diagnostic_type: DiagnosticType::SecurityHotspot,
    severity: DiagnosticSeverityLevel::Critical,
    scope: DiagnosticScope::Bsl,
    modules: &[bsl_metadata::ModuleType::CommonModule],
    minutes_to_fix: 15,
    activated_by_default: true,
    compatibility_mode: DiagnosticCompatibilityMode::Undefined,
    tags: &[MetadataTag::Badpractice, MetadataTag::Standard],
    can_locate_on_project: false,
    extra_min_for_complexity: 0.0,
    lsp_severity_override: "",
};

pub fn check(ctx: &DiagnosticsContext) -> Vec<Diagnostic> {
    let code = DiagnosticCode::ExecuteExternalCodeInCommonModule;

    if ctx.is_disabled_with_metadata(code) {
        return Vec::new();
    }

    let module = match common_module_helpers::find_common_module_for_file_anywhere(ctx) {
        Some(m) => m,
        None => return Vec::new(),
    };

    if !should_check_module(&module, ctx.config.ordinary_app_support) {
        return Vec::new();
    }

    detect_violations(ctx)
}

fn should_check_module(module: &bsl_metadata::CommonModule, ordinary_app_support: bool) -> bool {
    module.is_server()
        || module.is_external_connection()
        || (ordinary_app_support && module.is_client_ordinary_application())
}

fn detect_violations(ctx: &DiagnosticsContext) -> Vec<Diagnostic> {
    let code = DiagnosticCode::ExecuteExternalCodeInCommonModule;
    let parse = ctx.parse();
    let root = parse.syntax_node();
    let mut diagnostics = Vec::new();

    for node in root.descendants() {
        match node.kind() {
            SyntaxKind::EXECUTE_STMT => {
                diagnostics.push(create_diagnostic(code, node.text_range(), ctx));
            }
            SyntaxKind::CALL_EXPR if is_global_eval_call(&node) => {
                diagnostics.push(create_diagnostic(code, node.text_range(), ctx));
            }
            _ => {}
        }
    }

    diagnostics
}

fn create_diagnostic(
    code: DiagnosticCode,
    range: ide_db::TextRange,
    ctx: &DiagnosticsContext,
) -> Diagnostic {
    Diagnostic {
        code,
        message:
            "Execution of external code in a common module on a server is a potential vulnerability"
                .to_string(),
        severity: ctx.severity(code),
        range,
        tags: ctx.tags(code),
        fixes: vec![],
    }
}

fn is_global_eval_call(node: &syntax::SyntaxNode) -> bool {
    let first_token = match node.first_token() {
        Some(t) => t,
        None => return false,
    };

    if first_token.kind() != SyntaxKind::IDENT {
        return false;
    }

    if let Some(prev) = syntax::prev_token_past_empty(&first_token) {
        if prev.kind() == SyntaxKind::DOT {
            return false;
        }
    }

    bsl_platform::security::registry().lookup_global(first_token.text()).is_some_and(|e| {
        matches!(e.category, bsl_platform::security::Category::ExecuteExternalCode)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::*;
    use crate::DiagnosticsConfig;
    use expect_test::expect;
    use ide_db::base_db::{SourceDatabase, SourceRoot, SourceRootId};
    use ide_db::RootDatabaseImpl;
    use std::rc::Rc;
    use vfs::{FileId, FileSet, VfsPath};

    fn context_for(
        code: &str,
        config: DiagnosticsConfig,
    ) -> (RootDatabaseImpl, FileId, Rc<DiagnosticsConfig>) {
        let mut db = RootDatabaseImpl::new();

        let mut file_set = FileSet::default();
        let file_id = FileId(0);
        file_set.insert(file_id, VfsPath::new("/ОбменСКассой/Module.bsl"));

        let source_root = SourceRoot::new_local(file_set);
        db.set_source_root(SourceRootId(0), source_root);
        db.set_file_source_root(file_id, SourceRootId(0));
        db.set_file_text(file_id, code);

        (db, file_id, Rc::new(config))
    }

    fn check_violations_directly(code: &str) -> Vec<Diagnostic> {
        let (db, file_id, config) = context_for(code, DiagnosticsConfig::all_enabled());
        let provider = ide_db::SalsaProvider::new(&db, None);
        let ctx = DiagnosticsContext::new(&config, file_id, &provider);
        detect_violations(&ctx)
    }

    fn module(
        server: bool,
        external_connection: bool,
        ordinary_client: bool,
        managed_client: bool,
    ) -> bsl_metadata::CommonModule {
        bsl_metadata::CommonModule::builder()
            .name("ОбменСКассой")
            .server(server)
            .external_connection(external_connection)
            .client_ordinary_application(ordinary_client)
            .client_managed_application(managed_client)
            .build()
    }

    #[test]
    fn test_detect_execute_statement() {
        let code = r#"Процедура ПрименитьПравилоОкругления(ТекстПравила)
	Выполнить(ТекстПравила);
КонецПроцедуры

Функция СуммаПоПравилу(ТекстПравила, Чек)
	Возврат Вычислить(ТекстПравила);
КонецФункции

Функция СуммаБезПравила(Чек)
	Возврат ВычислитьИтог(Чек);
КонецФункции
"#;
        let diagnostics = check_violations_directly(code);
        expect![[r#"
            ExecuteExternalCodeInCommonModule @ 2:2..2:26
              message: Execution of external code in a common module on a server is a potential vulnerability
              severity: Warning
            ExecuteExternalCodeInCommonModule @ 6:10..6:33
              message: Execution of external code in a common module on a server is a potential vulnerability
              severity: Warning"#]].assert_eq(&format_diags(code, &diagnostics));
    }

    #[test]
    fn test_no_configuration_returns_empty() {
        let code = r#"Процедура ПрименитьПравилоОкругления(ТекстПравила)
	Выполнить(ТекстПравила);
КонецПроцедуры
"#;
        check_diagnostics_snapshot_for(
            code,
            DiagnosticCode::ExecuteExternalCodeInCommonModule,
            expect![[r#""#]],
        );
    }

    #[test]
    fn test_qualified_and_similar_names_ignored() {
        let code = r#"Функция СуммаПоПравилу(Касса, Чек)
	Промежуточная = Касса.Вычислить(Чек);
	Касса.Выполнить();
	Возврат ВычислитьИтог(Промежуточная);
КонецФункции
"#;
        let diagnostics = check_violations_directly(code);
        expect![[r#""#]].assert_eq(&format_diags(code, &diagnostics));
    }

    #[test]
    fn test_should_not_check_client_managed_module() {
        let managed = module(false, false, false, true);
        assert!(!should_check_module(&managed, true));
        assert!(!should_check_module(&managed, false));
    }

    #[test]
    fn test_should_check_server_module() {
        assert!(should_check_module(&module(true, false, false, false), false));
        assert!(should_check_module(&module(true, false, false, true), false));
    }

    #[test]
    fn test_should_check_external_connection_module() {
        assert!(should_check_module(&module(false, true, false, false), false));
        assert!(should_check_module(&module(false, true, false, true), false));
    }

    #[test]
    fn test_should_check_ordinary_client_module() {
        for managed in [false, true] {
            let ordinary = module(false, false, true, managed);
            assert!(should_check_module(&ordinary, true));
            assert!(!should_check_module(&ordinary, false));
        }
    }

    const CONFIGURED_SOURCE: &str = r#"Процедура РассчитатьНадбавку(Формула, Калькулятор)
	Выполнить(Формула);
	Надбавка = Вычислить(Формула);
	Калькулятор.Вычислить(Формула);
КонецПроцедуры
"#;

    fn check_configured_module(
        server: bool,
        external: bool,
        ordinary: bool,
        config: DiagnosticsConfig,
    ) -> Vec<Diagnostic> {
        let fixture = test_fixture::CfeFixtureBuilder::new("").build();
        let modules = fixture.root().join("CommonModules");
        let ext = modules.join("Caller/Ext");
        std::fs::create_dir_all(&ext).expect("create CommonModule directory");
        std::fs::write(ext.join("Module.bsl"), CONFIGURED_SOURCE).expect("write CommonModule body");
        std::fs::write(
            modules.join("Caller.xml"),
            format!(
                r#"<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses">
<CommonModule uuid="52000000-0000-0000-0000-000000000001"><Properties>
<Name>Caller</Name>
<ClientManagedApplication>true</ClientManagedApplication>
<Server>{server}</Server>
<ExternalConnection>{external}</ExternalConnection>
<ClientOrdinaryApplication>{ordinary}</ClientOrdinaryApplication>
</Properties></CommonModule>
</MetaDataObject>"#
            ),
        )
        .expect("write CommonModule metadata");
        check_with_cfe_config(CONFIGURED_SOURCE, fixture, config)
            .into_iter()
            .filter(|d| d.code == DiagnosticCode::ExecuteExternalCodeInCommonModule)
            .collect()
    }

    fn assert_configured_violations(diagnostics: &[Diagnostic]) {
        expect![[r#"
            ExecuteExternalCodeInCommonModule @ 2:2..2:21
              message: Execution of external code in a common module on a server is a potential vulnerability
              severity: Warning
            ExecuteExternalCodeInCommonModule @ 3:13..3:31
              message: Execution of external code in a common module on a server is a potential vulnerability
              severity: Warning"#]]
        .assert_eq(&format_diags(CONFIGURED_SOURCE, diagnostics));
    }

    #[test]
    fn configured_module_flags_select_external_code_diagnostics() {
        for ordinary_app_support in [false, true] {
            let config = DiagnosticsConfig { ordinary_app_support, ..Default::default() };
            assert!(check_configured_module(false, false, false, config.clone()).is_empty());
            assert_configured_violations(&check_configured_module(
                true,
                false,
                false,
                config.clone(),
            ));
            assert_configured_violations(&check_configured_module(
                false,
                true,
                false,
                config.clone(),
            ));
            let ordinary = check_configured_module(false, false, true, config);
            if ordinary_app_support {
                assert_configured_violations(&ordinary);
            } else {
                assert!(ordinary.is_empty());
            }
        }
    }

    #[test]
    fn configured_module_disabling_has_a_positive_control() {
        let mut config = DiagnosticsConfig::default();
        assert_configured_violations(&check_configured_module(true, false, false, config.clone()));
        config.disabled.push(DiagnosticCode::ExecuteExternalCodeInCommonModule);
        assert!(check_configured_module(true, false, false, config).is_empty());
    }

    #[test]
    fn test_disabled_config() {
        let code = "Процедура П(Т)\n\tВыполнить(Т);\nКонецПроцедуры\n";
        let mut config = DiagnosticsConfig::default();
        config.disabled.push(DiagnosticCode::ExecuteExternalCodeInCommonModule);
        let (db, file_id, config) = context_for(code, config);
        let provider = ide_db::SalsaProvider::new(&db, None);
        let ctx = DiagnosticsContext::new(&config, file_id, &provider);

        let diagnostics = check(&ctx);
        expect![[r#""#]].assert_eq(&format_diags(code, &diagnostics));
    }
}
