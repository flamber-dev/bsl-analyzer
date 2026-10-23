//! A constant's value manager (`КонстантаМенеджерЗначения.<Имя>`) in two places that missed
//! its members. Its value-manager module sees them as bare names (`ДополнительныеСвойства`,
//! `Значение`, `ОбменДанными`), and a data separator whose composition takes the constant — an
//! explicit `Use`, or `AutoUse = Use` without a `DontUse` — is a field of it. Both checked on
//! 8.3.17 and 8.3.27; every case keeps a control that the platform rejects.
//!
//! Configuration: `Константа2` is in `Разделитель1` by an explicit `Use` (`AutoUse = DontUse`);
//! `Разделитель2` has `AutoUse = Use` and lists `Константа3` with `DontUse`.

use ide::{Analysis, DiagnosticsConfig};
use ide_db::base_db::{SourceDatabase, SourceRoot, SourceRootId};
use ide_db::RootDatabaseImpl;
use std::path::Path;
use vfs::{FileId, FileSet, VfsPath};

const HEAD: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:cfg="http://v8.1c.ru/8.1/data/enterprise/current-config" version="2.10">"#;

fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn separator(name: &str, auto_use: &str, content: &str) -> String {
    format!(
        r#"{HEAD}
  <CommonAttribute uuid="0cb00000-0000-0000-0000-0000000003{n}">
    <Properties>
      <Name>{name}</Name>
      <Type>
        <v8:Type>xs:decimal</v8:Type>
        <v8:NumberQualifiers><v8:Digits>7</v8:Digits><v8:FractionDigits>0</v8:FractionDigits><v8:AllowedSign>Nonnegative</v8:AllowedSign></v8:NumberQualifiers>
      </Type>
      <Content>{content}</Content>
      <AutoUse>{auto_use}</AutoUse>
      <DataSeparation>Separate</DataSeparation>
      <SeparatedDataUse>IndependentlyAndSimultaneously</SeparatedDataUse>
    </Properties>
  </CommonAttribute>
</MetaDataObject>"#,
        n = &name[name.len() - 1..],
    )
}

fn content_item(constant: &str, usage: &str) -> String {
    format!(
        "<xr:Item><xr:Metadata>Constant.{constant}</xr:Metadata><xr:Use>{usage}</xr:Use><xr:ConditionalSeparation/></xr:Item>"
    )
}

fn configuration(root: &Path) {
    write(
        root,
        "Configuration.xml",
        &format!(
            r#"{HEAD}
  <Configuration uuid="0cb00000-0000-0000-0000-000000000301">
    <Properties><Name>КонтекстКонстант</Name></Properties>
  </Configuration>
</MetaDataObject>"#
        ),
    );
    for (i, name) in ["Константа1", "Константа2", "Константа3", "Константа4"].iter().enumerate()
    {
        write(
            root,
            &format!("Constants/{name}.xml"),
            &format!(
                r#"{HEAD}
  <Constant uuid="0cb00000-0000-0000-0000-00000000031{i}">
    <Properties>
      <Name>{name}</Name>
      <Type><v8:Type>xs:string</v8:Type><v8:StringQualifiers><v8:Length>10</v8:Length><v8:AllowedLength>Variable</v8:AllowedLength></v8:StringQualifiers></Type>
    </Properties>
  </Constant>
</MetaDataObject>"#
            ),
        );
    }
    write(
        root,
        "CommonAttributes/Разделитель1.xml",
        &separator("Разделитель1", "DontUse", &content_item("Константа2", "Use")),
    );
    write(
        root,
        "CommonAttributes/Разделитель2.xml",
        &separator("Разделитель2", "Use", &content_item("Константа3", "DontUse")),
    );
}

/// Every diagnostic message of `code` placed at `rel` (a path under the configuration root).
fn messages(rel: &str, code: &str) -> Vec<String> {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    configuration(root);
    write(root, rel, code);

    let file_id = FileId(0);
    let mut db = RootDatabaseImpl::new();
    db.set_all_config_paths(vec![(None, root.to_path_buf())], None);
    let mut file_set = FileSet::default();
    file_set.insert(file_id, VfsPath::new(root.join(rel).to_string_lossy().as_ref()));
    db.set_source_root(SourceRootId(0), SourceRoot::new_local(file_set));
    db.set_file_source_root(file_id, SourceRootId(0));
    db.set_file_text(file_id, code);

    Analysis::from_database(db)
        .diagnostics(file_id, &DiagnosticsConfig::all_enabled())
        .into_iter()
        .map(|d| format!("{:?}: {}", d.code, d.message))
        .collect()
}

fn mentioning(messages: &[String], name: &str) -> Vec<String> {
    messages.iter().filter(|m| m.contains(name)).cloned().collect()
}

#[test]
fn value_manager_module_sees_its_members_as_bare_names() {
    let found = messages(
        "Constants/Константа1/Ext/ValueManagerModule.bsl",
        "Процедура ПередЗаписью(Отказ)
    ДополнительныеСвойства.Вставить(\"Проба\", Значение);
    Если ОбменДанными.Загрузка Тогда
        Возврат;
    КонецЕсли;
КонецПроцедуры
",
    );
    assert!(mentioning(&found, "ДополнительныеСвойства").is_empty(), "{found:?}");
    assert!(mentioning(&found, "ОбменДанными").is_empty(), "{found:?}");
}

#[test]
fn value_manager_module_sees_its_separator_as_a_bare_name() {
    let found = messages(
        "Constants/Константа2/Ext/ValueManagerModule.bsl",
        "Процедура ПередЗаписью(Отказ)
    ДополнительныеСвойства.Вставить(\"Область\", Разделитель1);
КонецПроцедуры
",
    );
    assert!(mentioning(&found, "ДополнительныеСвойства").is_empty(), "{found:?}");
}

#[test]
fn value_manager_module_still_reports_a_name_it_lacks() {
    let found = messages(
        "Constants/Константа3/Ext/ValueManagerModule.bsl",
        "Процедура ПередЗаписью(Отказ)
    ДополнительныеСвойстваНет.Вставить(\"Проба\", 1);
КонецПроцедуры
",
    );
    assert_eq!(mentioning(&found, "ДополнительныеСвойстваНет").len(), 1, "{found:?}");
}

fn value_manager_field(constant: &str, separator: &str) -> Vec<String> {
    let found = messages(
        "CommonModules/Модуль1/Ext/Module.bsl",
        &format!(
            "Процедура Тест() Экспорт
    Менеджер = Константы.{constant}.СоздатьМенеджерЗначения();
    Менеджер.{separator} = 0;
    Значение = Менеджер.{separator};
КонецПроцедуры
"
        ),
    );
    mentioning(&found, separator)
}

#[test]
fn separator_is_a_field_of_a_constant_in_its_composition() {
    assert_eq!(value_manager_field("Константа2", "Разделитель1"), Vec::<String>::new());
    assert_eq!(value_manager_field("Константа4", "Разделитель2"), Vec::<String>::new());
}

#[test]
fn separator_is_still_missing_on_a_constant_outside_its_composition() {
    // The write and the read: `Константа3` is not listed by `Разделитель1` (`AutoUse = DontUse`)
    // and is listed by `Разделитель2` with `DontUse`.
    assert_eq!(value_manager_field("Константа3", "Разделитель1").len(), 2);
    assert_eq!(value_manager_field("Константа3", "Разделитель2").len(), 2);
}
