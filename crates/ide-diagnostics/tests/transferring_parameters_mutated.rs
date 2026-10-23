//! `TransferringParametersBetweenClientAndServer` for a parameter that the server method
//! changes in place (#271): the finding stays (std 487 p.12 does not recommend returning data
//! to the client through parameters), but the message names the remedy - return the data from
//! a function - and there is no `Знач` quick fix, because `Знач` would silently drop the data
//! the caller reads after the call. Every mutated case has a control of the same shape that
//! keeps the plain message and the quick fix.

use ide_db::base_db::{SourceDatabase, SourceRoot, SourceRootId};
use ide_db::RootDatabaseImpl;
use ide_diagnostics::{DiagnosticCode, DiagnosticsConfig};
use vfs::{FileId, FileSet, VfsPath};

const FORM_MODULE: &str = "/test/DataProcessors/Обработка1/Forms/Форма/Ext/Form/Module.bsl";

/// One line per finding: `<line>: <message> | fixes: <n>`.
fn findings(source: &str) -> String {
    let mut db = RootDatabaseImpl::new();
    let mut file_set = FileSet::default();
    file_set.insert(FileId(0), VfsPath::new(FORM_MODULE.to_string()));
    db.set_source_root(SourceRootId(0), SourceRoot::new_local(file_set));
    db.set_file_source_root(FileId(0), SourceRootId(0));
    db.set_file_text(FileId(0), source);
    let diagnostics =
        ide_diagnostics::file_diagnostics(&db, FileId(0), &DiagnosticsConfig::all_enabled());
    let mut out: Vec<(usize, String)> = diagnostics
        .iter()
        .filter(|d| d.code == DiagnosticCode::TransferringParametersBetweenClientAndServer)
        .map(|d| {
            let line = source[..usize::from(d.range.start())].matches('\n').count() + 1;
            (line, format!("{line}: {} | fixes: {}", d.message, d.fixes.len()))
        })
        .collect();
    out.sort();
    out.into_iter().map(|(_, s)| s).collect::<Vec<_>>().join("\n")
}

/// A client method calling `Сервер<n>(А)` for every server method in `servers`.
fn module(servers: &[&str]) -> String {
    let mut client = String::from("&НаКлиенте\nПроцедура Клиент()\n    А = Неопределено;\n");
    for index in 1..=servers.len() {
        client.push_str(&format!("    Сервер{index}(А);\n"));
    }
    client.push_str("КонецПроцедуры\n");
    let mut text = client;
    for (index, body) in servers.iter().enumerate() {
        text.push_str(&format!(
            "\n&НаСервереБезКонтекста\nПроцедура Сервер{}(П)\n{}\nКонецПроцедуры\n",
            index + 1,
            body
        ));
    }
    text
}

const MUTATED: &str = "возвращает данные на клиент: верните их результатом функции вместо изменения параметра | fixes: 0";
const PLAIN_FIXES: &str = "| fixes: 1";

fn assert_each(source: &str, expected_mutated: &[bool]) {
    let got = findings(source);
    let lines: Vec<&str> = got.lines().collect();
    assert_eq!(lines.len(), expected_mutated.len(), "findings:\n{got}");
    for (line, mutated) in lines.iter().zip(expected_mutated) {
        if *mutated {
            assert!(
                line.contains(MUTATED),
                "expected the mutated-parameter message: {line}\n{got}"
            );
        } else {
            assert!(
                line.contains("Установите модификатор \"Знач\"") && line.ends_with(PLAIN_FIXES),
                "expected the plain message with the Знач fix: {line}\n{got}"
            );
        }
    }
}

#[test]
fn field_assignment_marks_parameter_mutated() {
    let source = module(&[
        "    Если П.Переформировать Тогда\n        П.Переформировать = Ложь;\n    КонецЕсли;",
        "    Если П.Переформировать Тогда\n        Х = Ложь;\n    КонецЕсли;",
        "    П.Товары.Количество = 0;",
        "    Л = Новый Структура;\n    Л.Товары = П;",
    ]);
    assert_each(&source, &[true, false, true, false]);
}

#[test]
fn index_assignment_marks_parameter_mutated() {
    let source = module(&["    П[\"Ключ\"] = 1;", "    Х = П[\"Ключ\"];"]);
    assert_each(&source, &[true, false]);
}

#[test]
fn mutating_collection_method_marks_parameter_mutated() {
    let source = module(&[
        "    П.Добавить(1);",
        "    Х = П.Количество();",
        "    Строка = П.Добавить();\n    Строка.Сумма = 1;",
        "    П.Товары.Очистить();",
        "    П.Товары.Итог(\"Сумма\");",
        "    П.Insert(\"Ключ\", 1);",
        "    П.ЗагрузитьЗначения(Новый Массив);",
        "    П.Свернуть(\"Номенклатура\", \"Сумма\");",
    ]);
    assert_each(&source, &[true, false, true, true, false, true, true, true]);
}

#[test]
fn reassigned_parameter_still_has_no_finding() {
    let source = module(&["    П = Новый Структура;\n    П.Вставить(\"Ключ\", 1);"]);
    assert_eq!(findings(&source), "");
}

#[test]
fn mutation_through_local_alias_marks_parameter_mutated() {
    let source = module(&[
        "    Х = П;\n    Х.Добавить(1);",
        "    Х = П.Товары;\n    Х.Очистить();",
        "    Х = П[\"Ключ\"];\n    Х.Сумма = 1;",
        "    Х = П.Товары;\n    Н = Х.Количество();",
        "    Х = П;\n    Ю = Х;\n    Ю.Вставить(\"Ключ\", 1);",
    ]);
    assert_each(&source, &[true, true, true, false, true]);
}

#[test]
fn mutation_through_for_each_variable_marks_parameter_mutated() {
    let source = module(&[
        "    Для Каждого Стр Из П.Товары Цикл\n        Стр.Сумма = 0;\n    КонецЦикла;",
        "    Для Каждого Стр Из П.Товары Цикл\n        Х = Стр.Сумма;\n    КонецЦикла;",
    ]);
    assert_each(&source, &[true, false]);
}

#[test]
fn mutation_through_call_result_marks_parameter_mutated() {
    let source = module(&[
        "    П.Строки.Найти(1).Сумма = 5;",
        "    П.Получить(\"Товары\").Добавить(1);",
        "    Стр = П.Товары.Найти(1);\n    Стр.Сумма = 1;",
        "    Х = П.Скопировать();\n    Х.Добавить(1);",
        "    Х = П.Товары.Выгрузить();\n    Х.Очистить();",
    ]);
    assert_each(&source, &[true, true, true, false, false]);
}

#[test]
fn mutation_in_called_module_method_marks_parameter_mutated() {
    let mut source = module(&[
        "    Заполнить(П);",
        "    Прочитать(П);",
        "    Заполнить(П.Товары);",
        "    Сервер4(П);",
    ]);
    source.push_str(
        "\n&НаСервереБезКонтекста\nПроцедура Заполнить(Знач Т)\n    Вложенно(Т);\nКонецПроцедуры\n\
         \n&НаСервереБезКонтекста\nПроцедура Вложенно(Т)\n    Т.Вставить(\"Ключ\", 1);\nКонецПроцедуры\n\
         \n&НаСервереБезКонтекста\nПроцедура Прочитать(Т)\n    Х = Т.Количество();\nКонецПроцедуры\n",
    );
    assert_each(&source, &[true, false, true, false]);
}
