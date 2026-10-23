//! A template marker at the place of a parse error: the literal is substituted before it runs.
//!
//! Modules mark the place of a table name, a subquery or a field list with a parameter-like
//! `&Имя` where the query language takes no parameter, or with `{Имя}` / `{0}`, and replace
//! the marker with `СтрЗаменить` — often in another method. The platform rejects such a
//! literal as it is and executes the substituted text, so an error at a marker says nothing
//! about the query. A parameter where one is allowed, and an error away from a marker, keep
//! the literal checked.

use ide_db::base_db::{SourceDatabase, SourceRoot, SourceRootId};
use ide_db::RootDatabaseImpl;
use ide_diagnostics::{Diagnostic, DiagnosticCode, DiagnosticsConfig};
use vfs::{FileId, FileSet, VfsPath};

fn query_errors(source: &str) -> Vec<Diagnostic> {
    let mut db = RootDatabaseImpl::new();
    let file_id = FileId(0);
    let mut files = FileSet::default();
    files.insert(file_id, VfsPath::new("/test/Module.bsl".to_string()));
    db.set_source_root(SourceRootId(0), SourceRoot::new_local(files));
    db.set_file_source_root(file_id, SourceRootId(0));
    db.set_file_text(file_id, source);
    ide_diagnostics::file_diagnostics(&db, file_id, &DiagnosticsConfig::all_enabled())
        .into_iter()
        .filter(|diagnostic| diagnostic.code == DiagnosticCode::QueryParseError)
        .collect()
}

fn returned(text: &str) -> String {
    format!("Функция Тест() Экспорт\n\tВозврат \"{text}\";\nКонецФункции\n")
}

fn assert_silent(text: &str) {
    let source = returned(text);
    let errors = query_errors(&source);
    assert!(errors.is_empty(), "no QueryParseError expected in\n{source}\ngot {errors:#?}");
}

fn assert_reported(text: &str) {
    let source = returned(text);
    assert!(!query_errors(&source).is_empty(), "expected QueryParseError in\n{source}");
}

#[test]
fn a_parameter_where_the_language_takes_none_is_a_template_marker() {
    assert_silent("ВЫБРАТЬ Т.Ссылка ПОМЕСТИТЬ &Результат ИЗ Справочник.Валюты КАК Т");
    assert_silent("ВЫБРАТЬ 1 КАК А ПОМЕСТИТЬ ВТ1; УНИЧТОЖИТЬ &ИмяТаблицы");
    assert_silent("ВЫБРАТЬ Т.Ссылка ИЗ Справочник.&Имя КАК Т");
    assert_silent("ВЫБРАТЬ Т.Ссылка ИЗ (&ТекстПодзапроса) КАК Т");
    assert_silent(
        "ВЫБРАТЬ ВЫРАЗИТЬ(Т.Наименование КАК СТРОКА(&Длина)) КАК Н ИЗ Справочник.Валюты КАК Т",
    );
    assert_silent("ВЫБРАТЬ Т.Ссылка ИЗ Справочник.Валюты КАК &Псевдоним");
}

#[test]
fn a_brace_placeholder_is_a_template_marker() {
    assert_silent("ВЫБРАТЬ Т.Ссылка ИЗ Справочник.{0} КАК Т");
    assert_silent(
        "ВЫБРАТЬ
        |	{ОбщиеПоля}
        |	0 КАК Цена
        |ИЗ
        |	{Источник}",
    );
}

/// Controls, one detail away from the silent cases above.
#[test]
fn an_error_away_from_a_marker_is_reported() {
    // The parameter stands where one is allowed; the error is after it.
    assert_reported("ВЫБРАТЬ &Параметр КАК");
    assert_reported("ВЫБРАТЬ Т.Ссылка ИЗ Справочник.Валюты КАК Т ГДЕ &Условие И");
    // A block of the query language is not a placeholder: `{ГДЕ ...}` holds more than a word.
    assert_reported("ВЫБРАТЬ Т.Ссылка ИЗ Справочник.Валюты КАК Т {ГДЕ Т.Код} ГДЕ");
    // A name without the marker character is a table name the text misses, not a marker.
    assert_reported("ВЫБРАТЬ Т.Ссылка ИЗ Справочник. КАК Т");
}

/// A marker error does not cover a fault the substitution leaves in place.
#[test]
fn an_error_away_from_a_marker_is_reported_beside_a_marker_error() {
    assert_reported("ВЫБРАТЬ Т.Ссылка ПОМЕСТИТЬ &Результат ИЗ Справочник.Валюты КАК Т ГДЕ Т.Код =");
    assert_reported("ВЫБРАТЬ Т.Ссылка, ИЗ Справочник.&Имя КАК Т");
}
