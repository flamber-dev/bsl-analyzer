//! A documented metadata return type closes the result only while the body agrees with it.
//!
//! A function documented as returning `СправочникСсылка.X` whose body returns a structure, and a
//! function documenting an object the configuration does not have, used to make every field read
//! through the call an `UnresolvedField`, although the platform reads the field from the value the
//! function actually returns.

use hir::{HirDatabase, InferenceDiagnostic};
use ide_db::base_db::{SourceDatabase, SourceRoot, SourceRootId};
use ide_db::RootDatabaseImpl;
use std::path::PathBuf;
use vfs::{FileId, VfsPath};

fn designer_fixture_path() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../bsl-metadata/fixtures/designer"))
}

/// The object module of a data processor of a configuration whose catalog `Справочник1` has the
/// attributes `Реквизит1`..`Реквизит3`.
fn setup(text: &str) -> (RootDatabaseImpl, FileId) {
    let path =
        designer_fixture_path().join("DataProcessors/ТестоваяОбработка/Ext/ObjectModule.bsl");
    let file_id = FileId::from_raw(1);
    let mut db = RootDatabaseImpl::new();
    let mut file_set = vfs::FileSet::default();
    file_set.insert(file_id, VfsPath::new(path.to_string_lossy().to_string()));
    db.set_source_root(SourceRootId(0), SourceRoot::new_local(file_set));
    db.set_file_source_root(file_id, SourceRootId(0));
    db.set_file_text(file_id, text);
    db.set_all_config_paths(vec![(None, designer_fixture_path())], None);
    (db, file_id)
}

fn unresolved_fields(text: &str) -> Vec<String> {
    let (db, file_id) = setup(text);
    db.infer(file_id)
        .diagnostics
        .iter()
        .filter_map(|(_, diag)| match diag {
            InferenceDiagnostic::UnresolvedField { field_name, .. } => {
                Some(field_name.as_str().to_string())
            }
            _ => None,
        })
        .collect()
}

#[test]
fn documented_reference_over_a_returned_structure_does_not_close_the_result() {
    // The documentation was copied from a function returning a catalog reference; the body
    // returns a structure, and the caller reads its keys.
    let found = unresolved_fields(
        r#"// Возвращаемое значение:
//  СправочникСсылка.Справочник1 - найденное устройство.
Функция НайтиУстройство(Список)
	Устройство = Неопределено;
	Для Каждого Элемент Из Список Цикл
		Устройство = Новый Структура("КоличествоПодключенных, Параметры", 1, Неопределено);
	КонецЦикла;
	Возврат Устройство;
КонецФункции

Процедура Тест(Список) Экспорт
	Устройство = НайтиУстройство(Список);
	Устройство.КоличествоПодключенных = Устройство.КоличествоПодключенных + 1;
	П = Устройство.Параметры;
КонецПроцедуры
"#,
    );
    assert!(found.is_empty(), "the result holds the structure the body returns: {found:?}");
}

#[test]
fn documented_object_missing_from_the_configuration_does_not_close_the_result() {
    let found = unresolved_fields(
        r#"// Возвращаемое значение:
//  СправочникСсылка.НетТакогоСправочника - значение.
Функция Значение(Источник)
	Возврат Источник.Значение;
КонецФункции

Процедура Тест(Источник) Экспорт
	Х = Значение(Источник).Показатель;
КонецПроцедуры
"#,
    );
    assert!(found.is_empty(), "the documented catalog does not exist: {found:?}");
}

#[test]
fn documentation_the_body_agrees_with_still_closes_the_result() {
    // The body returns the documented reference (or nothing typed): the documentation stands,
    // and a field the catalog lacks is still a defect.
    let found = unresolved_fields(
        r#"// Возвращаемое значение:
//  СправочникСсылка.Справочник1 - ссылка.
Функция ПустойЭлемент(Признак)
	Если Признак Тогда
		Возврат Неопределено;
	КонецЕсли;
	Возврат Справочники.Справочник1.ПустаяСсылка();
КонецФункции

// Возвращаемое значение:
//  СправочникСсылка.Справочник1 - ссылка.
Функция ПереданныйЭлемент(Элемент)
	Возврат Элемент;
КонецФункции

Процедура Тест(Элемент) Экспорт
	А = ПустойЭлемент(Истина).Реквизит1;
	Б = ПустойЭлемент(Ложь).НетТакогоРеквизита;
	В = ПереданныйЭлемент(Элемент).ИТакогоНет;
КонецПроцедуры
"#,
    );
    assert_eq!(found, vec!["НетТакогоРеквизита".to_string(), "ИТакогоНет".to_string()]);
}
