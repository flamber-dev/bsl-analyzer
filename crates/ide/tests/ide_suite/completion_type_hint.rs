//! Member completion honours a `//: Имя = Выражение` type-hint comment on a
//! variable whose type inference cannot determine.

use ide::{Analysis, CompletionItem};
use ide_db::base_db::{SourceDatabase, SourceRoot, SourceRootId};
use ide_db::RootDatabaseImpl;
use std::path::PathBuf;
use test_fixture::Fixture;
use vfs::{FileId, FileSet};

fn designer_fixture_path() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../bsl-metadata/fixtures/designer"))
}

fn setup(fixture_text: &str, with_config: bool) -> (Analysis, FileId, u32) {
    let abs_idx = fixture_text.find("$0").expect("fixture must contain $0 cursor marker");
    let prefix = &fixture_text[..abs_idx];
    let header_start = prefix.rfind("//- ").expect("cursor must be inside a //- file");
    let header_end = prefix[header_start..].find('\n').expect("header newline") + header_start;
    let test_path = prefix[header_start + 4..header_end].to_string();
    let cursor = (abs_idx - (header_end + 1)) as u32;
    let cleaned = fixture_text.replacen("$0", "", 1);
    let fixture = Fixture::parse(&cleaned);

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
    if with_config {
        db.set_all_config_paths(vec![(None, designer_fixture_path())], None);
    }
    let test_file = fixture
        .files
        .iter()
        .find(|(_, f)| f.path.as_path().to_string_lossy().ends_with(&test_path))
        .map(|(id, _)| *id)
        .expect("cursor-bearing file not found");
    (Analysis::from_database(db), test_file, cursor)
}

fn complete(fixture: &str, with_config: bool) -> Vec<CompletionItem> {
    let (analysis, file_id, offset) = setup(fixture, with_config);
    analysis.completions(file_id, offset, None, ide::Locale::Ru)
}

fn has_label(items: &[CompletionItem], label: &str) -> bool {
    items.iter().any(|i| i.label == label)
}

fn labels(items: &[CompletionItem]) -> Vec<&str> {
    items.iter().map(|i| i.label.as_str()).collect()
}

#[test]
fn hint_on_untyped_parameter_offers_value_table_members() {
    let items = complete(
        r#"//- /test.bsl
Процедура Тест(Парам)
    //: Парам = Новый ТаблицаЗначений
    Парам.$0
КонецПроцедуры
"#,
        false,
    );
    for expected in ["Добавить", "ВыгрузитьКолонку"] {
        assert!(has_label(&items, expected), "`{expected}` expected; got: {:?}", labels(&items));
    }
}

#[test]
fn hint_on_loop_variable_typed_as_any_offers_value_table_members() {
    // The element of an untyped `Массив` is typed `Any` (the platform's
    // `Произвольный`), not `Unknown`, as are standard event-handler parameters; the
    // hint must override that too-generic type.
    let items = complete(
        r#"//- /test.bsl
Процедура Тест()
    Массив = Новый Массив;
    Для Каждого Элемент Из Массив Цикл
        //: Элемент = Новый ТаблицаЗначений
        Элемент.$0
    КонецЦикла;
КонецПроцедуры
"#,
        false,
    );
    for expected in ["Добавить", "ВыгрузитьКолонку"] {
        assert!(has_label(&items, expected), "`{expected}` expected; got: {:?}", labels(&items));
    }
}

#[test]
fn without_hint_untyped_parameter_does_not_offer_value_table_members() {
    let items = complete(
        r#"//- /test.bsl
Процедура Тест(Парам)
    Парам.$0
КонецПроцедуры
"#,
        false,
    );
    assert!(!has_label(&items, "ВыгрузитьКолонку"), "unexpected: {:?}", labels(&items));
}

#[test]
fn without_hint_loop_variable_typed_as_any_does_not_offer_value_table_members() {
    let items = complete(
        r#"//- /test.bsl
Процедура Тест()
    Массив = Новый Массив;
    Для Каждого Элемент Из Массив Цикл
        Элемент.$0
    КонецЦикла;
КонецПроцедуры
"#,
        false,
    );
    assert!(!has_label(&items, "ВыгрузитьКолонку"), "unexpected: {:?}", labels(&items));
}

#[test]
fn hint_for_another_variable_is_ignored() {
    let items = complete(
        r#"//- /test.bsl
Процедура Тест(Парам, Другая)
    //: Другая = Новый ТаблицаЗначений
    Парам.$0
КонецПроцедуры
"#,
        false,
    );
    assert!(!has_label(&items, "ВыгрузитьКолонку"), "unexpected: {:?}", labels(&items));
}

#[test]
#[cfg_attr(not(corpus_contract), ignore = "corpus contract: needs the platform help corpus")]
fn hint_with_configuration_object_expression_offers_object_members() {
    let items = complete(
        r#"//- /test.bsl
Процедура Тест(Парам)
    //: Парам = Справочники.Справочник1.СоздатьЭлемент()
    Парам.$0
КонецПроцедуры
"#,
        true,
    );
    assert!(has_label(&items, "ДополнительныеСвойства"), "got: {:?}", labels(&items));
}
