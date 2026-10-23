//! A value table built by its constructor is judged against the attested member surface of
//! the platform type `ТаблицаЗначений`: its columns are members of its rows, so the table's
//! own members are the platform's whatever columns it carries, and a member the platform
//! does not have fails at run time («Метод объекта не обнаружен», «Поле объекта не
//! обнаружено»).

use ide_db::base_db::{SourceDatabase, SourceRoot, SourceRootId};
use ide_db::{RootDatabaseImpl, SalsaProvider};
use ide_diagnostics::{Diagnostic, DiagnosticCode, DiagnosticsConfig, DiagnosticsContext};
use test_fixture::Fixture;

#[test]
#[cfg_attr(
    corpus_contract,
    ignore = "the corpus profile serves a versionless corpus: the completeness gate is shut by contract"
)]
fn a_missing_method_of_a_constructed_value_table_is_reported() {
    let messages = messages_of(
        "Процедура Тест()\n    Таблица = Новый ТаблицаЗначений;\n    Таблица.Прочитать();\nКонецПроцедуры",
        DiagnosticCode::UnresolvedMethodCall,
    );
    assert_eq!(messages, ["Метод 'Прочитать' не найден у 'ТаблицаЗначений'"]);
}

#[test]
#[cfg_attr(
    corpus_contract,
    ignore = "the corpus profile serves a versionless corpus: the completeness gate is shut by contract"
)]
fn a_missing_field_of_a_constructed_value_table_is_reported() {
    let messages = messages_of(
        "Процедура Тест()\n    Таблица = Новый ТаблицаЗначений();\n    Значение = Таблица.НетТакогоПоля;\nКонецПроцедуры",
        DiagnosticCode::UnresolvedField,
    );
    assert_eq!(messages.len(), 1, "expected one UnresolvedField, got: {messages:?}");
    assert!(messages[0].contains("НетТакогоПоля"), "the message names the field: {messages:?}");
}

/// The members the platform does have, the English spellings, and the row the table hands
/// out — whose columns are the code's own — never report.
#[test]
fn members_the_value_table_has_stay_silent() {
    let source = "Процедура Тест()
    Таблица = Новый ТаблицаЗначений;
    Таблица.Колонки.Добавить(\"Сумма\");
    Индексы = Таблица.Индексы;
    Строка = Таблица.Добавить();
    Строка.Сумма = 1;
    Всего = Таблица.Итог(\"Сумма\");
    Таблица.Свернуть(\"Сумма\");
    Количество = Таблица.Количество();
    Для Каждого СтрокаТаблицы Из Таблица Цикл
        СтрокаТаблицы.Сумма = 2;
    КонецЦикла;
    Table = New ValueTable;
    Table.Columns.Add(\"Amount\");
    Table.Clear();
КонецПроцедуры";
    assert!(member_findings(source).is_empty(), "{:?}", member_findings(source));
}

/// The walk types a local by its textually latest write; on the other path the variable
/// holds the caller's value, which may be anything. Neither that nor a parameter or a call
/// result is vouched for, so the check stays silent there.
#[test]
fn a_table_not_built_by_the_constructor_on_every_path_stays_silent() {
    let source = "Процедура Тест(Параметр, Условие)
    Таблица = Параметр;
    Если Условие Тогда
        Таблица = Новый ТаблицаЗначений;
    КонецЕсли;
    Таблица.Прочитать();
    Значение = Таблица.НетТакогоПоля;
    Выгруженная = Новый Запрос(\"ВЫБРАТЬ 1 КАК Поле\").Выполнить().Выгрузить();
    Выгруженная.Прочитать();
КонецПроцедуры";
    assert!(member_findings(source).is_empty(), "{:?}", member_findings(source));
}

fn member_findings(source: &str) -> Vec<String> {
    run_diagnostics(source)
        .into_iter()
        .filter(|d| {
            d.code == DiagnosticCode::UnresolvedMethodCall
                || d.code == DiagnosticCode::UnresolvedField
        })
        .map(|d| format!("{}: {}", d.code.as_str(), d.message))
        .collect()
}

fn messages_of(source: &str, code: DiagnosticCode) -> Vec<String> {
    run_diagnostics(source).into_iter().filter(|d| d.code == code).map(|d| d.message).collect()
}

fn run_diagnostics(source: &str) -> Vec<Diagnostic> {
    let fixture = Fixture::parse(&format!("//- /test.bsl\n{source}"));
    let mut db = RootDatabaseImpl::new();
    let mut file_set = vfs::FileSet::default();

    for (file_id, file) in &fixture.files {
        file_set.insert(*file_id, file.path.clone());
        db.set_file_text(*file_id, &file.content);
    }

    let source_root = SourceRoot::new_local(file_set);
    db.set_source_root(SourceRootId(0), source_root);
    for file_id in fixture.files.keys() {
        db.set_file_source_root(*file_id, SourceRootId(0));
    }

    let file_id = *fixture.files.keys().last().expect("fixture should contain a test file");
    let config = DiagnosticsConfig::all_enabled();
    let provider = SalsaProvider::new(&db, None);
    let ctx = DiagnosticsContext::new(&config, file_id, &provider);

    ide_diagnostics::diagnostics(&ctx)
}
