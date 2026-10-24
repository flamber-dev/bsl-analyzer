//! An information register record form: the common attributes that reach the register
//! are fields of its `Запись` too, and a register whose common-attribute membership
//! cannot be decided leaves the field set open.

use hir::{HirDatabase, InferenceDiagnostic, TypeKernelDb, TypeKind};
use ide_db::base_db::{SourceDatabase, SourceRoot, SourceRootId};
use ide_db::RootDatabaseImpl;
use std::path::{Path, PathBuf};
use vfs::{FileId, FileSet, VfsPath};

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("mkdir");
    for entry in std::fs::read_dir(from).expect("read_dir") {
        let entry = entry.expect("entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).expect("copy");
        }
    }
}

/// A copy of the common-attributes fixture with a record form on its information
/// register (`Организация`, the separator and `ОбщийКомментарий` all reach it), plus
/// optionally an unreadable common attribute that leaves every field set open.
fn common_attributes_root_with_record_form(unreadable: bool) -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("tempdir");
    copy_dir(
        &PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../bsl-metadata/fixtures/common_attributes"
        )),
        root.path(),
    );
    let form_dir = root.path().join("InformationRegisters/РегистрСведений1/Forms/ФормаЗаписи/Ext");
    std::fs::create_dir_all(form_dir.join("Form")).expect("mkdir form");
    let designer_form = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../bsl-metadata/fixtures/designer/InformationRegisters/РегистрСведений1/Forms/ФормаЗаписи/Ext/Form.xml"
    ));
    std::fs::copy(designer_form, form_dir.join("Form.xml")).expect("copy form");
    if unreadable {
        std::fs::write(root.path().join("CommonAttributes/Сломанный.xml"), "<not xml")
            .expect("write broken");
    }
    root
}

fn setup(root: &Path, bsl: &str) -> (RootDatabaseImpl, FileId) {
    let module =
        root.join("InformationRegisters/РегистрСведений1/Forms/ФормаЗаписи/Ext/Form/Module.bsl");
    let mut db = RootDatabaseImpl::new();
    let file_id = FileId(0);
    let mut file_set = FileSet::default();
    file_set.insert(file_id, VfsPath::new(module.to_string_lossy().as_ref()));
    db.set_source_root(SourceRootId(0), SourceRoot::new_local(file_set));
    db.set_file_source_root(file_id, SourceRootId(0));
    db.set_file_text(file_id, bsl);
    db.set_all_config_paths(vec![(None, root.to_path_buf())], None);
    (db, file_id)
}

fn unresolved_fields_at(root: &Path, bsl: &str) -> Vec<String> {
    let (db, file_id) = setup(root, bsl);
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

/// The record form's fields are the record manager's, so the common attributes that
/// reach the register are fields of `Запись` too, the separator among them.
#[test]
fn a_common_attribute_of_the_register_resolves_on_the_record() {
    let root = common_attributes_root_with_record_form(false);
    let bsl = "&НаСервере\nПроцедура Тест()\n    А = Запись.Организация;\n    \
               Б = Запись.ОбластьДанныхОсновныеДанные;\n    В = Запись.ОбщийКомментарий;\n    \
               Г = Запись.Измерение1;\n    Д = Запись.НетТакогоПоля;\nКонецПроцедуры\n";
    // The deliberate miss proves `Запись` is a closed receiver here.
    assert_eq!(unresolved_fields_at(root.path(), bsl), vec!["НетТакогоПоля".to_string()]);
}

/// Common-attribute membership the reader cannot decide leaves the register's field
/// set open, and the record form then stays silent like the record manager does.
#[test]
fn an_open_register_field_set_keeps_the_record_silent() {
    let root = common_attributes_root_with_record_form(true);
    let bsl = "&НаСервере\nПроцедура Тест()\n    З = Запись;\n    Д = Запись.НетТакогоПоля;\nКонецПроцедуры\n";
    assert!(unresolved_fields_at(root.path(), bsl).is_empty());
    // Silence alone would also follow from an untyped `Запись`; the record must still be typed.
    let (db, file_id) = setup(root.path(), bsl);
    let record = db.infer(file_id).var_types.get("з").copied().expect("З must be inferred");
    assert!(
        !matches!(db.lookup_type(record), TypeKind::Unknown),
        "Запись must stay typed when its field set is open"
    );
}
