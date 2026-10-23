use bsl_types::builders::Builders;
use bsl_types::kind::TypeId;
use bsl_types::testing::RootConfigCtx;
use hir_def::resolver::Resolver;
use hir_def::Name;

use crate::db::HirDatabase;

/// A member of the module's own object: of `ЭтотОбъект` in an object module, and of
/// the value manager in a constant's value-manager module. There `ДополнительныеСвойства`,
/// `Значение`, `ОбменДанными` and a separator the constant belongs to compile as bare names
/// on 8.3.17 and 8.3.27, while a name the value manager lacks is "Переменная не определена".
pub(crate) fn resolve_this_object_member(
    db: &dyn HirDatabase,
    resolver: &Resolver,
    name: &Name,
) -> Option<TypeId> {
    let (kind, mdo_name) = match crate::this_object::resolve_this_object_owner(db, resolver) {
        Some((mdo_type, mdo_name)) => {
            (hir_def::ty::MetadataKind::object_kind_for(mdo_type)?, mdo_name)
        }
        None => (
            hir_def::ty::MetadataKind::ConstantValueManager,
            crate::this_object::resolve_this_value_manager_owner(db, resolver)?,
        ),
    };
    let module_id = resolver.module_id()?;
    let obj_resolver = crate::object_resolver::DbObjectResolver::new(db, module_id.file_id);
    let receiver_id = db.metadata_ref(kind, mdo_name.as_str().to_string(), &RootConfigCtx);
    crate::field_lookup::lookup_field(db, &obj_resolver, receiver_id, name).map(|info| info.ty)
}

pub(crate) fn resolve_this_record_set_member(
    db: &dyn HirDatabase,
    resolver: &Resolver,
    name: &Name,
) -> Option<TypeId> {
    let (mdo_type, mdo_name) = crate::this_object::resolve_this_record_set_owner(db, resolver)?;
    let kind = hir_def::ty::MetadataKind::record_set_kind_for(mdo_type)?;
    let module_id = resolver.module_id()?;
    let obj_resolver = crate::object_resolver::DbObjectResolver::new(db, module_id.file_id);
    let receiver_id = db.metadata_ref(kind, mdo_name.as_str().to_string(), &RootConfigCtx);
    crate::field_lookup::lookup_field(db, &obj_resolver, receiver_id, name).map(|info| info.ty)
}
