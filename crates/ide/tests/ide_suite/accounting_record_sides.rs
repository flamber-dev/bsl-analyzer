//! A record of a correspondence accounting register keeps every non-balance dimension and
//! resource per side: `<Имя>Дт` / `<Имя>Кт` (`<Name>Dr` / `<Name>Cr`), and the bare name does
//! not exist on it. A balance member stays bare, and so does every member of a register without
//! correspondence. On 8.3.17 and 8.3.27 the platform accepts exactly that and rejects the other
//! spellings with "Поле объекта не обнаружено".

use ide::{Analysis, DiagnosticCode, DiagnosticsConfig};
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

/// `Измерение1` and `Ресурс1` are balance members, `Измерение2` and `Ресурс2` are not.
fn register(n: u8, correspondence: &str) -> String {
    let member = |tag: &str, id: u8, name: &str, ty: &str, balance: bool| {
        format!(
            "<{tag} uuid=\"0cb00000-0000-0000-0000-0000000004{n}{id}\"><Properties><Name>{name}</Name><Type><v8:Type>{ty}</v8:Type></Type><Balance>{balance}</Balance></Properties></{tag}>"
        )
    };
    format!(
        r#"{HEAD}
  <AccountingRegister uuid="0cb00000-0000-0000-0000-0000000004{n}0">
    <Properties>
      <Name>РегистрБухгалтерии{n}</Name>
      <ChartOfAccounts>ChartOfAccounts.ПланСчетов1</ChartOfAccounts>
      {correspondence}
    </Properties>
    <ChildObjects>{}{}{}{}</ChildObjects>
  </AccountingRegister>
</MetaDataObject>"#,
        member("Dimension", 1, "Измерение1", "xs:string", true),
        member("Dimension", 2, "Измерение2", "xs:string", false),
        member("Resource", 3, "Ресурс1", "xs:decimal", true),
        member("Resource", 4, "Ресурс2", "xs:decimal", false),
    )
}

/// `UnresolvedField` messages for `body` run against a fresh record of `РегистрБухгалтерии<n>`.
fn unresolved_fields(n: u8, body: &str) -> Vec<String> {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "Configuration.xml",
        &format!(
            r#"{HEAD}
  <Configuration uuid="0cb00000-0000-0000-0000-000000000401">
    <Properties><Name>ПроводкиПоСторонам</Name></Properties>
  </Configuration>
</MetaDataObject>"#
        ),
    );
    write(
        root,
        "ChartsOfAccounts/ПланСчетов1.xml",
        &format!(
            r#"{HEAD}
  <ChartOfAccounts uuid="0cb00000-0000-0000-0000-000000000402">
    <Properties><Name>ПланСчетов1</Name><MaxExtDimensionCount>0</MaxExtDimensionCount></Properties>
  </ChartOfAccounts>
</MetaDataObject>"#
        ),
    );
    write(
        root,
        "AccountingRegisters/РегистрБухгалтерии1.xml",
        &register(1, "<Correspondence>true</Correspondence>"),
    );
    write(
        root,
        "AccountingRegisters/РегистрБухгалтерии2.xml",
        &register(2, "<Correspondence>false</Correspondence>"),
    );
    write(root, "AccountingRegisters/РегистрБухгалтерии3.xml", &register(3, ""));

    let rel = "CommonModules/Модуль1/Ext/Module.bsl";
    let code = format!(
        "Процедура Тест() Экспорт
    Запись = РегистрыБухгалтерии.РегистрБухгалтерии{n}.СоздатьНаборЗаписей().Добавить();
{body}
КонецПроцедуры
"
    );
    write(root, rel, &code);

    let file_id = FileId(0);
    let mut db = RootDatabaseImpl::new();
    db.set_all_config_paths(vec![(None, root.to_path_buf())], None);
    let mut file_set = FileSet::default();
    file_set.insert(file_id, VfsPath::new(root.join(rel).to_string_lossy().as_ref()));
    db.set_source_root(SourceRootId(0), SourceRoot::new_local(file_set));
    db.set_file_source_root(file_id, SourceRootId(0));
    db.set_file_text(file_id, &code);

    Analysis::from_database(db)
        .diagnostics(file_id, &DiagnosticsConfig::all_enabled())
        .into_iter()
        .filter(|d| d.code == DiagnosticCode::UnresolvedField)
        .map(|d| d.message)
        .collect()
}

#[test]
fn correspondence_record_splits_non_balance_members_by_side() {
    let found = unresolved_fields(
        1,
        "    Запись.Измерение2Дт = \"\";
    Запись.Измерение2Кт = \"\";
    Запись.Ресурс2Дт = 1;
    Запись.Ресурс2Кт = 2;
    Значение = Запись.Измерение2Dr;
    Запись.Измерение1 = \"\";
    Запись.Ресурс1 = 3;",
    );
    assert_eq!(found, Vec::<String>::new());
}

#[test]
fn correspondence_record_rejects_bare_non_balance_and_sided_balance_members() {
    let found = unresolved_fields(
        1,
        "    Запись.Измерение2 = \"\";
    Значение = Запись.Ресурс2;
    Значение = Запись.Ресурс1Дт;",
    );
    assert_eq!(found.len(), 3, "{found:?}");
}

#[test]
fn record_without_correspondence_keeps_members_bare() {
    let found = unresolved_fields(
        2,
        "    Запись.Измерение2 = \"\";
    Запись.Ресурс2 = 1;",
    );
    assert_eq!(found, Vec::<String>::new());
    let found = unresolved_fields(2, "    Значение = Запись.Измерение2Дт;");
    assert_eq!(found.len(), 1, "{found:?}");
}

#[test]
fn record_of_unknown_correspondence_accepts_both_spellings() {
    let found = unresolved_fields(
        3,
        "    Запись.Измерение2 = \"\";
    Запись.Измерение2Дт = \"\";
    Запись.Ресурс2Кт = 1;",
    );
    assert_eq!(found, Vec::<String>::new());
}
