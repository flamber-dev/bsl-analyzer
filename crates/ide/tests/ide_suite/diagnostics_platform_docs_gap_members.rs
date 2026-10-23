//! Members the platform accepts although the help archive has no page for them:
//! `ПолучитьИмяПредопределенного` of the managers of objects with predefined
//! items, and the generic metadata-object methods of `Метаданные` itself
//! (`Представление`, `ПолноеИмя`, `Родитель`, `РасширениеКонфигурации`). Each
//! case keeps a control that differs by one detail and stays reported.

use bsl_platform::PlatformDataInner;
use hir::{HirDatabase, InferenceDiagnostic};
use ide_db::base_db::{SourceDatabase, SourceRoot, SourceRootId};
use ide_db::RootDatabaseImpl;
use std::path::PathBuf;
use test_fixture::Fixture;
use vfs::{FileId, FileSet};

fn designer_fixture_path() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../bsl-metadata/fixtures/designer"))
}

fn has_platform_data() -> bool {
    !PlatformDataInner::instance().all_methods().is_empty()
}

fn setup(fixture_text: &str) -> (RootDatabaseImpl, FileId) {
    let fixture = Fixture::parse(fixture_text);
    let mut db = RootDatabaseImpl::new();
    let mut file_set = FileSet::default();
    for (file_id, file) in &fixture.files {
        file_set.insert(*file_id, file.path.clone());
    }
    db.set_source_root(SourceRootId(0), SourceRoot::new_local(file_set));
    for (file_id, file) in &fixture.files {
        db.set_file_source_root(*file_id, SourceRootId(0));
        db.set_file_text(*file_id, &file.content);
    }
    db.set_all_config_paths(vec![(None, designer_fixture_path())], None);
    let test_file = fixture
        .files
        .iter()
        .find(|(_, f)| f.path.as_path().to_string_lossy().ends_with("/test.bsl"))
        .map(|(id, _)| *id)
        .expect("fixture must contain /test.bsl");
    (db, test_file)
}

fn unresolved_methods(db: &RootDatabaseImpl, file_id: FileId) -> Vec<String> {
    db.infer(file_id)
        .diagnostics
        .iter()
        .filter_map(|(_, d)| match d {
            InferenceDiagnostic::UnresolvedMethodCall { method_name, .. } => {
                Some(method_name.as_str().to_owned())
            }
            _ => None,
        })
        .collect()
}

/// `ПолучитьИмяПредопределенного` runs on 8.3.17 and 8.3.27 on the managers of
/// a catalog, a chart of characteristic types, a chart of accounts and a chart
/// of calculation types (an empty ref gives an empty string); a document
/// manager has no such method and the platform says "Метод объекта не
/// обнаружен".
#[test]
fn predefined_item_name_resolves_on_managers_with_predefined_items() {
    if !has_platform_data() {
        return;
    }
    let (db, file_id) = setup(
        r#"
//- /test.bsl
Процедура Тест()
    Имя1 = Справочники.Справочник1.ПолучитьИмяПредопределенного(Справочники.Справочник1.ПустаяСсылка());
    Имя2 = ПланыВидовХарактеристик.ПланВидовХарактеристик1.ПолучитьИмяПредопределенного(Неопределено);
    Имя3 = ПланыСчетов.ПланСчетов1.ПолучитьИмяПредопределенного(Неопределено);
    Имя4 = ПланыВидовРасчета.ПланВидовРасчета1.ПолучитьИмяПредопределенного(Неопределено);
    Имя5 = Справочники.Справочник1.GetPredefinedItemName(Неопределено);
КонецПроцедуры
"#,
    );
    let unresolved = unresolved_methods(&db, file_id);
    assert!(unresolved.is_empty(), "unexpected UnresolvedMethodCall: {unresolved:?}");
}

#[test]
fn predefined_item_name_stays_unresolved_on_document_manager() {
    if !has_platform_data() {
        return;
    }
    let (db, file_id) = setup(
        r#"
//- /test.bsl
Процедура Тест()
    Имя1 = Документы.Документ1.ПолучитьИмяПредопределенного(Неопределено);
    Имя2 = Справочники.Справочник1.ПолучитьИмяПредопределенногоНет(Неопределено);
КонецПроцедуры
"#,
    );
    let unresolved = unresolved_methods(&db, file_id);
    assert_eq!(
        unresolved,
        vec![
            "ПолучитьИмяПредопределенного".to_owned(),
            "ПолучитьИмяПредопределенногоНет".to_owned()
        ],
        "the document manager and a misspelled name must stay reported"
    );
}

/// `Метаданные.Представление()` gives "Конфигурация", `ПолноеИмя()` gives
/// "Конфигурация.Конфигурация", `Родитель()` and `РасширениеКонфигурации()`
/// give `Неопределено` on 8.3.17 and 8.3.27; a misspelled name fails with
/// "Метод объекта не обнаружен".
#[test]
fn configuration_metadata_object_has_generic_metadata_methods() {
    if !has_platform_data() {
        return;
    }
    let (db, file_id) = setup(
        r#"
//- /test.bsl
Процедура Тест()
    П1 = Метаданные.Представление();
    П2 = Метаданные.ПолноеИмя();
    П3 = Метаданные.Родитель();
    П4 = Метаданные.РасширениеКонфигурации();
    П5 = Metadata.Presentation();
КонецПроцедуры
"#,
    );
    let unresolved = unresolved_methods(&db, file_id);
    assert!(unresolved.is_empty(), "unexpected UnresolvedMethodCall: {unresolved:?}");
}

#[test]
fn configuration_metadata_object_misspelled_method_stays_unresolved() {
    if !has_platform_data() {
        return;
    }
    let (db, file_id) = setup(
        r#"
//- /test.bsl
Процедура Тест()
    П1 = Метаданные.ПредставлениеНет();
КонецПроцедуры
"#,
    );
    let unresolved = unresolved_methods(&db, file_id);
    assert_eq!(unresolved, vec!["ПредставлениеНет".to_owned()]);
}
