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

#[test]
fn alias_stops_after_it_is_assigned_another_value() {
    let source = module(&[
        "    Х = П.Товары;\n    Х = Новый Массив;\n    Х.Добавить(1);",
        "    Для Каждого Х Из П.Товары Цикл\n    КонецЦикла;\n    Х = Новый Структура;\n    Х.Вставить(\"А\", 1);",
        "    Х = П.Товары;\n    Х.Очистить();\n    Х = Новый Массив;",
        "    Х = Новый Массив;\n    Если Истина Тогда\n        Х = П.Товары;\n    КонецЕсли;\n    Х.Очистить();",
        "    Х = Новый Массив;\n    Пока Истина Цикл\n        Х.Очистить();\n        Х = П.Товары;\n    КонецЦикла;",
    ]);
    assert_each(&source, &[false, false, true, true, true]);
}

#[test]
fn called_method_changes_parameter_before_reassigning_it() {
    let mut source = module(&["    Заполнить(П);", "    Заменить(П);"]);
    source.push_str(
        "\n&НаСервереБезКонтекста\nПроцедура Заполнить(Т)\n    Т.Добавить(1);\n    Т = Новый Массив;\nКонецПроцедуры\n\
         \n&НаСервереБезКонтекста\nПроцедура Заменить(Т)\n    Т = Новый Массив;\n    Т.Добавить(1);\nКонецПроцедуры\n",
    );
    assert_each(&source, &[true, false]);
}

#[test]
fn mutually_recursive_methods_share_the_change() {
    let source = module(&["    Сервер2(П);\n    П.Добавить(1);", "    Сервер1(П);"]);
    assert_each(&source, &[true, true]);
}

#[test]
fn fill_property_values_marks_its_target_mutated() {
    let source = module(&[
        "    ЗаполнитьЗначенияСвойств(П, Новый Структура(\"А\", 1));",
        "    Х = Новый Структура(\"А\");\n    ЗаполнитьЗначенияСвойств(Х, П);",
    ]);
    assert_each(&source, &[true, false]);
}

#[test]
fn parameter_stored_in_a_container_is_changed_through_it() {
    let source = module(&[
        "    Л = Новый Структура;\n    Л.Товары = П;\n    Л.Товары.Очистить();",
        "    Л = Новый Структура(\"Данные\", П);\n    Л.Данные.Вставить(\"К\", 1);",
        "    Л = Новый Структура;\n    Л.Вставить(\"Данные\", П);\n    Л.Данные.Вставить(\"К\", 1);",
        "    Л = Новый Структура(\"Данные\", П);\n    Л.Вставить(\"К\", 1);",
    ]);
    assert_each(&source, &[true, true, true, false]);
}

#[test]
fn module_function_result_carries_the_parameter() {
    let mut source = module(&[
        "    Т = ПолучитьТовары(П);\n    Т.Очистить();",
        "    ПолучитьТовары(П).Очистить();",
        "    Т = НовыеТовары(П);\n    Т.Очистить();",
    ]);
    source.push_str(
        "\n&НаСервереБезКонтекста\nФункция ПолучитьТовары(Док)\n    Возврат Док.Товары;\nКонецФункции\n\
         \n&НаСервереБезКонтекста\nФункция НовыеТовары(Док)\n    Возврат Новый Массив;\nКонецФункции\n",
    );
    assert_each(&source, &[true, true, false]);
}

#[test]
fn alias_survives_jumps_out_of_order() {
    let source = module(&[
        "    Х = П.Товары;\n    Перейти ~К;\n    Х = Новый Массив;\n    ~К:\n    Х.Очистить();",
        "    Попытка\n        Х = П;\n        ВызватьИсключение \"ошибка\";\n        Х = Новый Массив;\n    Исключение\n        Х.Очистить();\n    КонецПопытки;",
        "    Пока Истина Цикл\n        Если П.Флаг Тогда\n            Х = П.Товары;\n            Прервать;\n        КонецЕсли;\n        Х = Новый Массив;\n    КонецЦикла;\n    Х.Очистить();",
        "    Т = ?(П.Флаг, П.Товары, Новый Массив);\n    Т.Очистить();",
        "    Т = ?(П.Флаг, Новый Массив, Новый Массив);\n    Т.Очистить();",
    ]);
    assert_each(&source, &[true, true, true, true, false]);
}

#[test]
fn module_variable_carries_the_parameter_between_methods() {
    let mut source = format!(
        "Перем Буфер;\n\n{}",
        module(&[
            "    Буфер = П.Товары;\n    ОчиститьБуфер();",
            "    Запомнить(П);\n    Буфер.Очистить();",
            "    Буфер = П.Товары;\n    Буфер = Новый Массив;\n    ОчиститьБуфер();",
        ])
    );
    source.push_str(
        "\n&НаСервереБезКонтекста\nПроцедура ОчиститьБуфер()\n    Буфер.Очистить();\nКонецПроцедуры\n\
         \n&НаСервереБезКонтекста\nПроцедура Запомнить(Т)\n    Буфер = Т;\nКонецПроцедуры\n",
    );
    assert_each(&source, &[true, true, false]);
}
