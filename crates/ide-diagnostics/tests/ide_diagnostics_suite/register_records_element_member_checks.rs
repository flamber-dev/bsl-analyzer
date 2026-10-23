//! The element of `Для Каждого Набор Из <Документ>.Движения` is the record set of a register
//! the document writes to, so a method no such record set has fails at run time («Метод
//! объекта не обнаружен») exactly as on a record set the manager creates.

use std::path::Path;

use ide_db::base_db::{SourceDatabase, SourceRoot, SourceRootId};
use ide_db::RootDatabaseImpl;
use ide_diagnostics::{DiagnosticCode, DiagnosticsConfig};
use vfs::{FileId, FileSet, VfsPath};

const HEADER: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xs="http://www.w3.org/2001/XMLSchema" version="2.20">"#;

const COMMON_MODULE: &str = "CommonModules/ОбщийМодуль1/Ext/Module.bsl";
const OBJECT_MODULE: &str = "Documents/Документ1/Ext/ObjectModule.bsl";
const RECORD_SET_MODULE: &str = "InformationRegisters/Регистр1/Ext/RecordSetModule.bsl";
const RECORD_SET_SOURCE: &str = "Процедура Пересчитать() Экспорт\nКонецПроцедуры\n";

#[test]
fn a_missing_method_on_the_element_of_a_document_s_movements_is_reported() {
    let found = method_misses(
        COMMON_MODULE,
        "Процедура Тест() Экспорт
    Объект = Документы.Документ1.СоздатьДокумент();
    Для Каждого Набор Из Объект.Движения Цикл
        Набор.Читать();
    КонецЦикла;
КонецПроцедуры
",
    );
    assert_eq!(
        found,
        ["Метод 'Читать' не найден у 'РегистрыНакопления.Регистр2, РегистрыСведений.Регистр1' | Набор.Читать"]
    );
}

#[test]
fn a_document_writing_to_one_register_names_that_record_set() {
    let found = method_misses(
        COMMON_MODULE,
        "Процедура Тест() Экспорт
    Объект = Документы.Документ2.СоздатьДокумент();
    Для Каждого Набор Из Объект.Движения Цикл
        Набор.Читать();
    КонецЦикла;
КонецПроцедуры
",
    );
    assert_eq!(found, ["Метод 'Читать' не найден у 'РегистрыНакопления.Регистр2' | Набор.Читать"]);
}

/// In the document's own object module the collection is the bare `Движения` or
/// `ЭтотОбъект.Движения`.
#[test]
fn the_object_module_s_own_movements_are_checked_too() {
    let found = method_misses(
        OBJECT_MODULE,
        "Процедура ОбработкаПроведения(Отказ, РежимПроведения)
    Для Каждого Набор Из Движения Цикл
        Набор.Читать();
    КонецЦикла;
    Для Каждого Набор Из ЭтотОбъект.Движения Цикл
        Набор.Прочитать();
        Набор.НетТакогоМетода();
    КонецЦикла;
КонецПроцедуры
",
    );
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(found[0].ends_with("| Набор.Читать"), "{found:?}");
    assert!(found[1].ends_with("| Набор.НетТакогоМетода"), "{found:?}");
}

/// The members every record set has, a method only one register's record set module
/// exports (an arm owns it, so the call may well succeed), a document that writes to no
/// register and a collection whose document is unknown all stay silent.
#[test]
fn members_some_record_set_has_and_unknown_documents_stay_silent() {
    let found = method_misses(
        COMMON_MODULE,
        "Процедура Тест(Параметр) Экспорт
    Объект = Документы.Документ1.СоздатьДокумент();
    Для Каждого Набор Из Объект.Движения Цикл
        Набор.Прочитать();
        Набор.Записать();
        Набор.Очистить();
        Количество = Набор.Количество();
        Набор.Пересчитать();
    КонецЦикла;
    Для Каждого Набор Из Объект.RegisterRecords Цикл
        Набор.Read();
    КонецЦикла;
    БезДвижений = Документы.Документ3.СоздатьДокумент();
    Для Каждого Набор Из БезДвижений.Движения Цикл
        Набор.Читать();
    КонецЦикла;
    Для Каждого Набор Из Параметр.Движения Цикл
        Набор.Читать();
    КонецЦикла;
КонецПроцедуры
",
    );
    assert!(found.is_empty(), "{found:?}");
}

fn method_misses(relative: &str, source: &str) -> Vec<String> {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("cf");
    write_configuration(&root);
    write(&root, relative, source);

    let mut db = RootDatabaseImpl::new();
    let file_id = FileId(0);
    let mut files = FileSet::default();
    for (index, (relative, text)) in
        [(relative, source), (RECORD_SET_MODULE, RECORD_SET_SOURCE)].into_iter().enumerate()
    {
        let id = FileId(index as u32);
        files.insert(id, VfsPath::new(root.join(relative).to_string_lossy().into_owned()));
        db.set_file_text(id, text);
    }
    db.set_source_root(SourceRootId(0), SourceRoot::new_local(files));
    db.set_file_source_root(FileId(0), SourceRootId(0));
    db.set_file_source_root(FileId(1), SourceRootId(0));
    db.set_all_config_paths(
        vec![(None, root.clone())],
        Some(bsl_metadata::ExtensionScope::InfoBase),
    );
    db.set_workspace_load_complete(true);

    ide_diagnostics::file_diagnostics(&db, file_id, &DiagnosticsConfig::all_enabled())
        .into_iter()
        .filter(|d| d.code == DiagnosticCode::UnresolvedMethodCall)
        .map(|d| format!("{} | {}", d.message, &source[d.range]))
        .collect()
}

fn write(root: &Path, relative: &str, text: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn write_configuration(root: &Path) {
    write(
        root,
        "Configuration.xml",
        &format!(
            r#"{HEADER}
    <Configuration uuid="5a5a0000-0000-0000-0000-000000000001">
        <Properties><Name>Конфигурация1</Name></Properties>
        <ChildObjects>
            <CommonModule>ОбщийМодуль1</CommonModule>
            <Document>Документ1</Document>
            <Document>Документ2</Document>
            <Document>Документ3</Document>
            <InformationRegister>Регистр1</InformationRegister>
            <AccumulationRegister>Регистр2</AccumulationRegister>
        </ChildObjects>
    </Configuration>
</MetaDataObject>"#
        ),
    );
    write(
        root,
        "CommonModules/ОбщийМодуль1.xml",
        &format!(
            r#"{HEADER}
    <CommonModule uuid="5a5a0000-0000-0000-0000-000000000002">
        <Properties><Name>ОбщийМодуль1</Name><Server>true</Server></Properties>
    </CommonModule>
</MetaDataObject>"#
        ),
    );
    document(
        root,
        "Документ1",
        3,
        &["InformationRegister.Регистр1", "AccumulationRegister.Регистр2"],
    );
    document(root, "Документ2", 4, &["AccumulationRegister.Регистр2"]);
    document(root, "Документ3", 5, &[]);
    write(
        root,
        "InformationRegisters/Регистр1.xml",
        &format!(
            r#"{HEADER}
    <InformationRegister uuid="5a5a0000-0000-0000-0000-000000000006">
        <Properties><Name>Регистр1</Name><InformationRegisterPeriodicity>Nonperiodical</InformationRegisterPeriodicity><WriteMode>RecorderSubordinate</WriteMode></Properties>
    </InformationRegister>
</MetaDataObject>"#
        ),
    );
    write(root, RECORD_SET_MODULE, RECORD_SET_SOURCE);
    write(
        root,
        "AccumulationRegisters/Регистр2.xml",
        &format!(
            r#"{HEADER}
    <AccumulationRegister uuid="5a5a0000-0000-0000-0000-000000000007">
        <Properties><Name>Регистр2</Name><RegisterType>Balance</RegisterType></Properties>
    </AccumulationRegister>
</MetaDataObject>"#
        ),
    );
}

fn document(root: &Path, name: &str, uuid: u32, registers: &[&str]) {
    let items: String = registers
        .iter()
        .map(|r| format!("\n                <xr:Item xsi:type=\"xr:MDObjectRef\">{r}</xr:Item>"))
        .collect();
    write(
        root,
        &format!("Documents/{name}.xml"),
        &format!(
            r#"{HEADER}
    <Document uuid="5a5a0000-0000-0000-0000-{uuid:012}">
        <Properties>
            <Name>{name}</Name>
            <RegisterRecords>{items}
            </RegisterRecords>
        </Properties>
    </Document>
</MetaDataObject>"#
        ),
    );
}
