use crate::enums::{ModuleType, ObjectBelonging, ReturnValueReuse};
use crate::traits::{MdObject, Module};
use serde::{Deserialize, Serialize};
use std::any::Any;
use uuid::Uuid;

/// A common module.
///
/// The properties read from XML are stored as `Option`: `None` means the
/// element is absent, so a borrowed module of an extension inherits it from the
/// base module (see [`CommonModule::apply_extension_overlay`]). Getters keep the
/// standalone defaults for absent properties.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommonModule {
    #[serde(rename = "uuid")]
    uuid: Uuid,

    #[serde(rename = "name")]
    name: String,

    #[serde(rename = "comment", default, skip_serializing_if = "Option::is_none")]
    comment: Option<String>,

    #[serde(rename = "uri", default, skip_serializing_if = "Option::is_none")]
    uri: Option<String>,

    #[serde(rename = "objectBelonging", default)]
    object_belonging: ObjectBelonging,

    #[serde(rename = "extendedConfigurationObject", default)]
    extended_configuration_object: Option<Uuid>,

    #[serde(rename = "protected", default)]
    protected: bool,

    #[serde(rename = "server", default, skip_serializing_if = "Option::is_none")]
    server: Option<bool>,

    #[serde(rename = "global", default, skip_serializing_if = "Option::is_none")]
    global: Option<bool>,

    #[serde(rename = "clientManagedApplication", default, skip_serializing_if = "Option::is_none")]
    client_managed_application: Option<bool>,

    #[serde(
        rename = "clientOrdinaryApplication",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    client_ordinary_application: Option<bool>,

    #[serde(rename = "externalConnection", default, skip_serializing_if = "Option::is_none")]
    external_connection: Option<bool>,

    #[serde(rename = "serverCall", default, skip_serializing_if = "Option::is_none")]
    server_call: Option<bool>,

    #[serde(rename = "privileged", default, skip_serializing_if = "Option::is_none")]
    privileged: Option<bool>,

    #[serde(rename = "returnValuesReuse", default, skip_serializing_if = "Option::is_none")]
    return_values_reuse: Option<ReturnValueReuse>,
}

impl CommonModule {
    pub fn builder() -> CommonModuleBuilder {
        CommonModuleBuilder::default()
    }

    pub fn return_values_reuse(&self) -> ReturnValueReuse {
        self.return_values_reuse.unwrap_or_default()
    }

    pub fn extends_uuid(&self) -> Option<&Uuid> {
        self.extended_configuration_object.as_ref()
    }

    /// Whether this module is an extension's adopted copy of `base`. `base` may
    /// itself be an overlay result, which carries the overlay's identity: a later
    /// extension in the chain still references the original module, so an
    /// adopted `base` is matched by the module it extends.
    pub fn adopts(&self, base: &CommonModule) -> bool {
        let base_origin = match base.object_belonging {
            ObjectBelonging::Adopted => base.extends_uuid().unwrap_or(&base.uuid),
            _ => &base.uuid,
        };
        self.object_belonging == ObjectBelonging::Adopted
            && stdx::case::eq_ignore_case(&self.name, &base.name)
            && Some(base_origin) == self.extends_uuid()
    }

    pub fn is_server(&self) -> bool {
        self.server.unwrap_or(false)
    }

    pub fn is_global(&self) -> bool {
        self.global.unwrap_or(false)
    }

    pub fn is_client_managed_application(&self) -> bool {
        self.client_managed_application.unwrap_or(false)
    }

    pub fn is_client_ordinary_application(&self) -> bool {
        self.client_ordinary_application.unwrap_or(false)
    }

    pub fn is_external_connection(&self) -> bool {
        self.external_connection.unwrap_or(false)
    }

    pub fn is_server_call(&self) -> bool {
        self.server_call.unwrap_or(false)
    }

    pub fn is_privileged(&self) -> bool {
        self.privileged.unwrap_or(false)
    }

    /// Apply an extension overlay (a borrowed module of the same name) onto this
    /// base module. Identity, URI and protection come from the overlay, as when
    /// the overlay replaced the module wholesale; each XML property absent in
    /// the overlay is inherited from the base, an explicit one wins.
    pub fn apply_extension_overlay(&mut self, overlay: &CommonModule) {
        let inherited = std::mem::replace(self, overlay.clone());
        self.server = self.server.or(inherited.server);
        self.global = self.global.or(inherited.global);
        self.client_managed_application =
            self.client_managed_application.or(inherited.client_managed_application);
        self.client_ordinary_application =
            self.client_ordinary_application.or(inherited.client_ordinary_application);
        self.external_connection = self.external_connection.or(inherited.external_connection);
        self.server_call = self.server_call.or(inherited.server_call);
        self.privileged = self.privileged.or(inherited.privileged);
        self.return_values_reuse = self.return_values_reuse.or(inherited.return_values_reuse);
    }

    pub fn set_uri(&mut self, uri: Option<String>) {
        self.uri = uri;
    }

    pub fn set_protected(&mut self, protected: bool) {
        self.protected = protected;
    }

    /// Heap bytes owned by this module, memoised by `ide-db`'s
    /// `parse_common_module_query` for Salsa's `heap_size` hook: its name plus
    /// the optional comment/URI strings. New heap-owning fields must be added
    /// here too.
    pub fn estimated_heap_size(&self) -> usize {
        self.name.capacity()
            + self.comment.as_ref().map_or(0, String::capacity)
            + self.uri.as_ref().map_or(0, String::capacity)
    }
}

impl MdObject for CommonModule {
    fn uuid(&self) -> &Uuid {
        &self.uuid
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn comment(&self) -> Option<&str> {
        self.comment.as_deref()
    }

    fn object_belonging(&self) -> ObjectBelonging {
        self.object_belonging
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Module for CommonModule {
    fn module_type(&self) -> ModuleType {
        ModuleType::CommonModule
    }

    fn uri(&self) -> Option<&str> {
        self.uri.as_deref()
    }

    fn is_protected(&self) -> bool {
        self.protected
    }
}

#[derive(Debug, Default)]
pub struct CommonModuleBuilder {
    uuid: Option<Uuid>,
    name: Option<String>,
    comment: Option<String>,
    uri: Option<String>,
    object_belonging: ObjectBelonging,
    extended_configuration_object: Option<Uuid>,
    protected: bool,
    server: Option<bool>,
    global: Option<bool>,
    client_managed_application: Option<bool>,
    client_ordinary_application: Option<bool>,
    external_connection: Option<bool>,
    server_call: Option<bool>,
    privileged: Option<bool>,
    return_values_reuse: Option<ReturnValueReuse>,
}

impl CommonModuleBuilder {
    pub fn uuid(mut self, uuid: Uuid) -> Self {
        self.uuid = Some(uuid);
        self
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn comment(mut self, comment: impl Into<String>) -> Self {
        self.comment = Some(comment.into());
        self
    }

    pub fn uri(mut self, uri: Option<impl Into<String>>) -> Self {
        self.uri = uri.map(|s| s.into());
        self
    }

    pub fn return_values_reuse(mut self, reuse: ReturnValueReuse) -> Self {
        self.return_values_reuse = Some(reuse);
        self
    }

    pub fn object_belonging(mut self, belonging: ObjectBelonging) -> Self {
        self.object_belonging = belonging;
        self
    }

    pub fn extends_uuid(mut self, uuid: Uuid) -> Self {
        self.extended_configuration_object = Some(uuid);
        self
    }

    pub fn extended_configuration_object(mut self, uuid: Option<Uuid>) -> Self {
        self.extended_configuration_object = uuid;
        self
    }

    pub fn server(mut self, server: bool) -> Self {
        self.server = Some(server);
        self
    }

    pub fn global(mut self, global: bool) -> Self {
        self.global = Some(global);
        self
    }

    pub fn privileged(mut self, privileged: bool) -> Self {
        self.privileged = Some(privileged);
        self
    }

    pub fn client_managed_application(mut self, value: bool) -> Self {
        self.client_managed_application = Some(value);
        self
    }

    pub fn client_ordinary_application(mut self, value: bool) -> Self {
        self.client_ordinary_application = Some(value);
        self
    }

    pub fn external_connection(mut self, value: bool) -> Self {
        self.external_connection = Some(value);
        self
    }

    pub fn server_call(mut self, value: bool) -> Self {
        self.server_call = Some(value);
        self
    }

    pub fn protected(mut self, value: bool) -> Self {
        self.protected = value;
        self
    }

    pub fn build(self) -> CommonModule {
        CommonModule {
            uuid: self.uuid.unwrap_or_else(Uuid::new_v4),
            name: self.name.unwrap_or_default(),
            comment: self.comment,
            uri: self.uri,
            object_belonging: self.object_belonging,
            extended_configuration_object: self.extended_configuration_object,
            protected: self.protected,
            server: self.server,
            global: self.global,
            client_managed_application: self.client_managed_application,
            client_ordinary_application: self.client_ordinary_application,
            external_connection: self.external_connection,
            server_call: self.server_call,
            privileged: self.privileged,
            return_values_reuse: self.return_values_reuse,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_true_module() -> CommonModule {
        CommonModule::builder()
            .name("Сервер")
            .server(true)
            .global(true)
            .client_managed_application(true)
            .client_ordinary_application(true)
            .external_connection(true)
            .server_call(true)
            .privileged(true)
            .build()
    }

    fn set_bool(builder: CommonModuleBuilder, index: usize, value: bool) -> CommonModuleBuilder {
        match index {
            0 => builder.server(value),
            1 => builder.global(value),
            2 => builder.client_managed_application(value),
            3 => builder.client_ordinary_application(value),
            4 => builder.external_connection(value),
            5 => builder.server_call(value),
            _ => builder.privileged(value),
        }
    }

    fn bool_values(module: &CommonModule) -> [bool; 7] {
        [
            module.is_server(),
            module.is_global(),
            module.is_client_managed_application(),
            module.is_client_ordinary_application(),
            module.is_external_connection(),
            module.is_server_call(),
            module.is_privileged(),
        ]
    }

    #[test]
    fn test_common_module_builder() {
        let module = CommonModule::builder()
            .name("ТестовыйМодуль")
            .return_values_reuse(ReturnValueReuse::DuringRequest)
            .server(true)
            .global(true)
            .build();

        assert_eq!(module.name(), "ТестовыйМодуль");
        assert_eq!(module.return_values_reuse(), ReturnValueReuse::DuringRequest);
        assert!(module.is_server());
        assert!(module.is_global());
        assert_eq!(module.module_type(), ModuleType::CommonModule);
    }

    #[test]
    fn test_md_object_trait() {
        let module = CommonModule::builder().name("TestModule").build();

        assert_eq!(module.name(), "TestModule");
        assert_eq!(module.object_belonging(), ObjectBelonging::Own);
    }

    #[test]
    fn extension_metadata_overlay_inherits_only_absent_common_module_properties() {
        let base_uuid = Uuid::new_v4();
        let overlay_uuid = Uuid::new_v4();
        let mut base = CommonModule::builder()
            .uuid(base_uuid)
            .name("Сервер")
            .uri(Some("base/CommonModules/Сервер/Ext/Module.bsl"))
            .protected(true)
            .server(true)
            .global(true)
            .client_managed_application(true)
            .client_ordinary_application(true)
            .external_connection(true)
            .server_call(true)
            .privileged(true)
            .return_values_reuse(ReturnValueReuse::DontUse)
            .build();
        let overlay = CommonModule::builder()
            .uuid(overlay_uuid)
            .name("Сервер")
            .uri(Some("extension/CommonModules/Сервер/Ext/Module.bsl"))
            .global(false)
            .protected(false)
            .build();

        base.apply_extension_overlay(&overlay);

        assert_eq!(base.uuid(), &overlay_uuid);
        assert_eq!(base.uri(), Some("extension/CommonModules/Сервер/Ext/Module.bsl"));
        assert!(!base.is_protected());
        assert!(base.is_server());
        assert!(!base.is_global(), "an explicit false in the extension wins");
        assert!(base.is_client_managed_application());
        assert!(base.is_client_ordinary_application());
        assert!(base.is_external_connection());
        assert!(base.is_server_call());
        assert!(base.is_privileged());
        assert_eq!(base.return_values_reuse(), ReturnValueReuse::DontUse);

        let protected_overlay_uuid = Uuid::new_v4();
        let mut unprotected_base = CommonModule::builder()
            .name("Сервер")
            .uri(Some("base/CommonModules/Сервер/Ext/Module.bsl"))
            .protected(false)
            .build();
        let protected_overlay = CommonModule::builder()
            .uuid(protected_overlay_uuid)
            .name("Сервер")
            .protected(true)
            .build();
        unprotected_base.apply_extension_overlay(&protected_overlay);
        assert_eq!(unprotected_base.uuid(), &protected_overlay_uuid);
        assert!(unprotected_base.is_protected());
        assert_eq!(
            unprotected_base.uri(),
            None,
            "overlay-local URI absence must not inherit the base URI"
        );

        let mut false_base = CommonModule::builder()
            .name("Сервер")
            .server(false)
            .global(false)
            .client_managed_application(false)
            .client_ordinary_application(false)
            .external_connection(false)
            .server_call(false)
            .privileged(false)
            .return_values_reuse(ReturnValueReuse::DontUse)
            .build();
        let true_overlay = CommonModule::builder()
            .name("Сервер")
            .server(true)
            .global(true)
            .client_managed_application(true)
            .client_ordinary_application(true)
            .external_connection(true)
            .server_call(true)
            .privileged(true)
            .return_values_reuse(ReturnValueReuse::DuringSession)
            .build();
        false_base.apply_extension_overlay(&true_overlay);
        assert!(false_base.is_server());
        assert!(false_base.is_global());
        assert!(false_base.is_client_managed_application());
        assert!(false_base.is_client_ordinary_application());
        assert!(false_base.is_external_connection());
        assert!(false_base.is_server_call());
        assert!(false_base.is_privileged());
        assert_eq!(false_base.return_values_reuse(), ReturnValueReuse::DuringSession);

        for omitted in 0..7 {
            let mut builder = CommonModule::builder().name("Сервер");
            for index in 0..7 {
                if index != omitted {
                    builder = set_bool(builder, index, false);
                }
            }
            let mut effective = all_true_module();
            effective.apply_extension_overlay(&builder.build());
            for (index, value) in bool_values(&effective).into_iter().enumerate() {
                assert_eq!(
                    value,
                    index == omitted,
                    "omitted bool index {omitted}, checked {index}"
                );
            }
        }
    }

    #[test]
    fn extension_metadata_serde_roundtrip_distinguishes_absent_false_and_unknown() {
        let absent = CommonModule::builder().name("Отсутствует").build();
        let explicit = CommonModule::builder()
            .name("Задано")
            .server(false)
            .global(false)
            .client_managed_application(false)
            .client_ordinary_application(false)
            .external_connection(false)
            .server_call(false)
            .privileged(false)
            .return_values_reuse(ReturnValueReuse::Unknown)
            .build();

        let absent_json = serde_json::to_string(&absent).unwrap();
        let explicit_json = serde_json::to_string(&explicit).unwrap();
        for property in [
            "server",
            "global",
            "clientManagedApplication",
            "clientOrdinaryApplication",
            "externalConnection",
            "serverCall",
            "privileged",
            "returnValuesReuse",
        ] {
            assert!(!absent_json.contains(property), "{property} must stay absent");
            assert!(explicit_json.contains(property), "{property} must stay explicit");
        }

        let absent_roundtrip: CommonModule = serde_json::from_str(&absent_json).unwrap();
        let explicit_roundtrip: CommonModule = serde_json::from_str(&explicit_json).unwrap();
        assert_eq!(absent_roundtrip, absent);
        assert_eq!(explicit_roundtrip, explicit);
        assert_eq!(explicit_roundtrip.server, Some(false));
        assert_eq!(explicit_roundtrip.return_values_reuse, Some(ReturnValueReuse::Unknown));
        assert!(!absent_roundtrip.is_server());
        assert!(!absent_roundtrip.is_global());
        assert!(!absent_roundtrip.is_client_managed_application());
        assert!(!absent_roundtrip.is_client_ordinary_application());
        assert!(!absent_roundtrip.is_external_connection());
        assert!(!absent_roundtrip.is_server_call());
        assert!(!absent_roundtrip.is_privileged());
        assert_eq!(absent_roundtrip.return_values_reuse(), ReturnValueReuse::Unknown);

        for enabled in 0..7 {
            let mut builder = CommonModule::builder().name("Смешанный");
            for index in 0..7 {
                builder = set_bool(builder, index, index == enabled);
            }
            let source = builder.build();
            let json = serde_json::to_string(&source).unwrap();
            let roundtrip: CommonModule = serde_json::from_str(&json).unwrap();
            assert_eq!(roundtrip, source);
            for (index, value) in bool_values(&roundtrip).into_iter().enumerate() {
                assert_eq!(
                    value,
                    index == enabled,
                    "enabled bool index {enabled}, checked {index}"
                );
            }
        }

        for reuse in [
            ReturnValueReuse::DontUse,
            ReturnValueReuse::DuringRequest,
            ReturnValueReuse::DuringSession,
        ] {
            let known = CommonModule::builder()
                .name("Известно")
                .server(true)
                .global(true)
                .client_managed_application(true)
                .client_ordinary_application(true)
                .external_connection(true)
                .server_call(true)
                .privileged(true)
                .return_values_reuse(reuse)
                .build();
            let json = serde_json::to_string(&known).unwrap();
            let roundtrip: CommonModule = serde_json::from_str(&json).unwrap();
            assert_eq!(roundtrip, known);
            assert_eq!(roundtrip.return_values_reuse(), reuse);
        }
    }
}
