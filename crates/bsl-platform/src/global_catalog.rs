use once_cell::sync::OnceCell;
use rustc_hash::{FxHashMap, FxHashSet};
use smol_str::SmolStr;
use std::fmt;
use std::str::FromStr;
use stdx::case::CaseExt;

use crate::{
    generated, ContextAvailability, PlatformDataInner, RawPlatformGlobalKind,
    RawPlatformGlobalSymbol,
};

static PLATFORM_GLOBAL_CATALOG: OnceCell<PlatformGlobalCatalog> = OnceCell::new();

/// Coverage of the exact platform-global name surface for a selected target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformCatalogStatus {
    Missing,
    Complete,
    Unverified,
    /// The bundled manifest is complete for another platform release line.
    UnsupportedTarget,
}

/// Numeric 1C platform release. Three- and four-component spellings are accepted;
/// catalog coverage is attested for the release line (major.minor.patch), while a
/// configured build number remains available for reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlatformVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub build: Option<u32>,
}

impl PlatformVersion {
    pub fn same_release(self, other: Self) -> bool {
        (self.major, self.minor, self.patch) == (other.major, other.minor, other.patch)
    }

    /// Lenient parse of a catalog "available since" value or a configured release:
    /// `8.0`, `8.3.6`, `8.3.17.1549`, and the help's suffixed spelling
    /// `8.3.6 (в режиме совместимости с версией 8.3.6 и последующими)` — the base
    /// is the text before ` (`. Two to four numeric parts; missing ones are 0.
    /// Anything else is `None`, which callers treat as "no version known".
    pub fn parse_catalog(value: &str) -> Option<Self> {
        let base = value.split_once(" (").map_or(value, |(base, _)| base).trim();
        let parts = base.split('.').collect::<Vec<_>>();
        if !(2..=4).contains(&parts.len()) {
            return None;
        }
        let number = |part: &str| {
            if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
                return None;
            }
            part.parse::<u32>().ok()
        };
        let optional = |index: usize| match parts.get(index) {
            Some(part) => number(part).map(Some),
            None => Some(None),
        };
        Some(Self {
            major: number(parts[0])?,
            minor: number(parts[1])?,
            patch: optional(2)?.unwrap_or(0),
            build: optional(3)?,
        })
    }

    /// Whether this release line (major.minor.patch) is later than `other`'s; the
    /// build number never makes a member "newer" — catalog versions carry none.
    pub fn release_newer_than(self, other: Self) -> bool {
        (self.major, self.minor, self.patch) > (other.major, other.minor, other.patch)
    }

    /// A configuration's compatibility mode, as `Configuration.xml` spells it
    /// (`Version8_2_13`, `DontUse`) or as a person writes it in a project setting
    /// (`8.2.13`, `Версия8_2_13`, `НеИспользовать`). `Some(None)` is "no mode": the
    /// platform's own behavior, nothing hidden. `None` is a value that is not a mode.
    pub fn parse_compatibility_mode(value: &str) -> Option<Option<Self>> {
        let value = value.trim();
        if matches!(value.to_lowercase().as_str(), "dontuse" | "неиспользовать" | "none")
        {
            return Some(None);
        }
        let digits = value
            .strip_prefix("Version")
            .or_else(|| value.strip_prefix("Версия"))
            .map(|rest| rest.replace('_', "."));
        let version = Self::parse_catalog(digits.as_deref().unwrap_or(value))?;
        Some(Some(version))
    }
}

impl FromStr for PlatformVersion {
    type Err = PlatformVersionParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let parts = value.split('.').collect::<Vec<_>>();
        if !(parts.len() == 3 || parts.len() == 4) {
            return Err(PlatformVersionParseError);
        }
        let number = |part: &str| {
            if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(PlatformVersionParseError);
            }
            part.parse::<u32>().map_err(|_| PlatformVersionParseError)
        };
        Ok(Self {
            major: number(parts[0])?,
            minor: number(parts[1])?,
            patch: number(parts[2])?,
            build: parts.get(3).map(|part| number(part)).transpose()?,
        })
    }
}

impl fmt::Display for PlatformVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(build) = self.build {
            write!(formatter, ".{build}")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformVersionParseError;

impl fmt::Display for PlatformVersionParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("expected a numeric 1C platform version such as 8.3.27 or 8.3.27.1644")
    }
}

impl std::error::Error for PlatformVersionParseError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformGlobalKind {
    Function,
    Property,
    SystemEnum,
}

/// Lower-layer-neutral ways in which a platform symbol may be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformSymbolCapabilities {
    pub callable: Option<bool>,
    pub readable_as_value: Option<bool>,
    pub assignable: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformGlobalSymbol {
    pub canonical_ru: SmolStr,
    pub canonical_en: SmolStr,
    pub kind: PlatformGlobalKind,
    pub capabilities: PlatformSymbolCapabilities,
    pub context: Option<ContextAvailability>,
    pub min_version: Option<SmolStr>,
    /// Declared value type for a property, or the canonical platform type for a
    /// system enumeration. Functions have no value type in this catalog.
    pub value_type: Option<SmolStr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformGlobalCatalogMetadata {
    pub schema_version: u32,
    pub platform_version: &'static str,
    pub edt_version: &'static str,
    pub global_context_sha256: &'static str,
    pub system_enums_sha256: &'static str,
}

/// Immutable, process-wide exact index. Membership comes only from the attested
/// EDT manifest; HBK help data enriches matching entries but cannot add names.
pub struct PlatformGlobalCatalog {
    status: PlatformCatalogStatus,
    metadata: Option<PlatformGlobalCatalogMetadata>,
    symbols: Vec<PlatformGlobalSymbol>,
    by_name: FxHashMap<SmolStr, usize>,
    ambiguous_names: FxHashSet<SmolStr>,
}

impl PlatformGlobalCatalog {
    pub fn instance() -> &'static Self {
        PLATFORM_GLOBAL_CATALOG.get_or_init(|| Self::build(PlatformDataInner::instance()))
    }

    /// Status of the bundled artifact itself. Use [`Self::status_for_target`] when
    /// deciding whether a miss proves absence for a project.
    pub fn status(&self) -> PlatformCatalogStatus {
        self.status
    }

    pub fn status_for_target(&self, target: Option<&str>) -> PlatformCatalogStatus {
        if self.status != PlatformCatalogStatus::Complete {
            return self.status;
        }
        let Some(catalog_version) = self.catalog_version() else {
            return PlatformCatalogStatus::Unverified;
        };
        let target_version = match target {
            Some(value) => match value.parse::<PlatformVersion>() {
                Ok(version) => version,
                Err(_) => return PlatformCatalogStatus::UnsupportedTarget,
            },
            None => catalog_version,
        };
        if target_version.same_release(catalog_version) {
            PlatformCatalogStatus::Complete
        } else {
            PlatformCatalogStatus::UnsupportedTarget
        }
    }

    pub fn catalog_version(&self) -> Option<PlatformVersion> {
        self.metadata?.platform_version.parse().ok()
    }

    pub fn metadata(&self) -> Option<PlatformGlobalCatalogMetadata> {
        self.metadata
    }

    pub fn symbols(&self) -> &[PlatformGlobalSymbol] {
        &self.symbols
    }

    /// Returns the deterministic first symbol for an alias. EDT contains a few
    /// intentional compatibility aliases/collisions; their presence is exposed by
    /// [`Self::ambiguous_names`], but it must never turn a known global into a miss.
    pub fn lookup(&self, name: &str) -> Option<&PlatformGlobalSymbol> {
        let key = SmolStr::from(name.fold_lower());
        self.by_name.get(&key).and_then(|&idx| self.symbols.get(idx))
    }

    pub fn contains(&self, name: &str) -> bool {
        let key = SmolStr::from(name.fold_lower());
        self.by_name.contains_key(&key)
    }

    pub fn ambiguous_names(&self) -> &FxHashSet<SmolStr> {
        &self.ambiguous_names
    }

    fn build(platform: &PlatformDataInner) -> Self {
        let raw_metadata = generated::PLATFORM_GLOBAL_CATALOG_METADATA;
        let complete = raw_metadata.is_some_and(|metadata| {
            metadata.complete_global_context && metadata.complete_system_enums
        });
        let mut catalog = Self {
            status: if raw_metadata.is_none() || generated::PLATFORM_GLOBAL_CATALOG.is_empty() {
                PlatformCatalogStatus::Missing
            } else if complete {
                PlatformCatalogStatus::Complete
            } else {
                PlatformCatalogStatus::Unverified
            },
            metadata: raw_metadata.map(|metadata| PlatformGlobalCatalogMetadata {
                schema_version: metadata.schema_version,
                platform_version: metadata.platform_version,
                edt_version: metadata.edt_version,
                global_context_sha256: metadata.global_context_sha256,
                system_enums_sha256: metadata.system_enums_sha256,
            }),
            symbols: Vec::with_capacity(generated::PLATFORM_GLOBAL_CATALOG.len()),
            by_name: FxHashMap::default(),
            ambiguous_names: FxHashSet::default(),
        };

        for raw in generated::PLATFORM_GLOBAL_CATALOG {
            catalog.push(Self::materialize(raw, platform));
        }
        catalog
    }

    fn materialize(
        raw: &RawPlatformGlobalSymbol,
        platform: &PlatformDataInner,
    ) -> PlatformGlobalSymbol {
        let context = Some(context_from_mask(raw.environment_mask));
        let (kind, capabilities, min_version, value_type) = match raw.kind {
            RawPlatformGlobalKind::Function => {
                let details = platform
                    .get_global_function(raw.canonical_ru)
                    .or_else(|| platform.get_global_function(raw.canonical_en));
                (
                    PlatformGlobalKind::Function,
                    PlatformSymbolCapabilities {
                        callable: Some(true),
                        readable_as_value: Some(false),
                        assignable: Some(false),
                    },
                    details.and_then(|function| function.min_version.clone()),
                    None,
                )
            }
            RawPlatformGlobalKind::Property => {
                let details = platform
                    .get_global_property(raw.canonical_ru)
                    .or_else(|| platform.get_global_property(raw.canonical_en));
                (
                    PlatformGlobalKind::Property,
                    PlatformSymbolCapabilities {
                        callable: Some(false),
                        readable_as_value: Some(true),
                        assignable: Some(raw.writable),
                    },
                    details.and_then(|property| property.min_version.clone()),
                    details.and_then(|property| property.property_types.first().cloned()),
                )
            }
            RawPlatformGlobalKind::SystemEnum => {
                let details = platform
                    .get_type(raw.canonical_ru)
                    .or_else(|| platform.get_type(raw.canonical_en));
                (
                    PlatformGlobalKind::SystemEnum,
                    PlatformSymbolCapabilities {
                        callable: Some(false),
                        readable_as_value: Some(true),
                        assignable: Some(false),
                    },
                    details.and_then(|ty| ty.min_version.clone()),
                    Some(SmolStr::new(if raw.canonical_ru.is_empty() {
                        raw.canonical_en
                    } else {
                        raw.canonical_ru
                    })),
                )
            }
        };
        PlatformGlobalSymbol {
            canonical_ru: raw.canonical_ru.into(),
            canonical_en: raw.canonical_en.into(),
            kind,
            capabilities,
            context,
            min_version,
            value_type,
        }
    }

    fn push(&mut self, symbol: PlatformGlobalSymbol) {
        let idx = self.symbols.len();
        let aliases = [symbol.canonical_ru.clone(), symbol.canonical_en.clone()];
        self.symbols.push(symbol);

        for alias in aliases.iter().filter(|alias| !alias.is_empty()) {
            let key = SmolStr::from(alias.as_str().fold_lower());
            match self.by_name.get(&key).copied() {
                None => {
                    self.by_name.insert(key, idx);
                }
                Some(previous) if previous == idx => {}
                Some(_) => {
                    self.ambiguous_names.insert(key);
                }
            }
        }
    }
}

fn context_from_mask(mask: u8) -> ContextAvailability {
    ContextAvailability {
        thick_client: mask & 1 != 0,
        thin_client: mask & 2 != 0,
        web_client: mask & 4 != 0,
        server: mask & 8 != 0,
        mobile_client: mask & 16 != 0,
        external_connection: mask & 32 != 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_parser_accepts_release_and_build() {
        assert_eq!(
            "8.3.27".parse(),
            Ok(PlatformVersion { major: 8, minor: 3, patch: 27, build: None })
        );
        assert_eq!(
            "8.3.27.1644".parse(),
            Ok(PlatformVersion { major: 8, minor: 3, patch: 27, build: Some(1644) })
        );
        assert!("8.3".parse::<PlatformVersion>().is_err());
        assert!("8.3.next".parse::<PlatformVersion>().is_err());
    }

    #[test]
    fn catalog_version_parser_takes_the_help_spellings() {
        let v = |major, minor, patch, build| PlatformVersion { major, minor, patch, build };
        assert_eq!(PlatformVersion::parse_catalog("8.0"), Some(v(8, 0, 0, None)));
        assert_eq!(PlatformVersion::parse_catalog("8.3.6"), Some(v(8, 3, 6, None)));
        assert_eq!(PlatformVersion::parse_catalog("8.3.17.1549"), Some(v(8, 3, 17, Some(1549))));
        assert_eq!(
            PlatformVersion::parse_catalog(
                "8.3.6 (в режиме совместимости с версией 8.3.6 и последующими)"
            ),
            Some(v(8, 3, 6, None))
        );
        assert_eq!(PlatformVersion::parse_catalog("8"), None);
        assert_eq!(PlatformVersion::parse_catalog(""), None);
        assert_eq!(PlatformVersion::parse_catalog("8.3.x"), None);
        assert_eq!(PlatformVersion::parse_catalog("8.3.1.2.3"), None);

        let min = PlatformVersion::parse_catalog("8.3.17.1549").unwrap();
        assert!(PlatformVersion::parse_catalog("8.3.18").unwrap().release_newer_than(min));
        assert!(!PlatformVersion::parse_catalog("8.3.17").unwrap().release_newer_than(min));
        assert!(!PlatformVersion::parse_catalog("8.2").unwrap().release_newer_than(min));
    }

    #[test]
    fn compatibility_mode_parser_takes_xml_and_setting_spellings() {
        let v =
            |major, minor, patch| Some(Some(PlatformVersion { major, minor, patch, build: None }));
        assert_eq!(PlatformVersion::parse_compatibility_mode("Version8_2_13"), v(8, 2, 13));
        assert_eq!(PlatformVersion::parse_compatibility_mode("Version8_3_17"), v(8, 3, 17));
        assert_eq!(PlatformVersion::parse_compatibility_mode("Версия8_3_6"), v(8, 3, 6));
        assert_eq!(PlatformVersion::parse_compatibility_mode("8.3.10"), v(8, 3, 10));
        assert_eq!(PlatformVersion::parse_compatibility_mode("DontUse"), Some(None));
        assert_eq!(PlatformVersion::parse_compatibility_mode("НеИспользовать"), Some(None));
        assert_eq!(PlatformVersion::parse_compatibility_mode("Version8_3_x"), None);
        assert_eq!(PlatformVersion::parse_compatibility_mode(""), None);
    }

    #[test]
    fn attested_catalog_is_target_aware() {
        let catalog = PlatformGlobalCatalog::instance();
        assert_eq!(catalog.status(), PlatformCatalogStatus::Complete);
        assert_eq!(catalog.status_for_target(None), PlatformCatalogStatus::Complete);
        assert_eq!(catalog.status_for_target(Some("8.3.27.1644")), PlatformCatalogStatus::Complete);
        assert_eq!(
            catalog.status_for_target(Some("8.3.28")),
            PlatformCatalogStatus::UnsupportedTarget
        );
        let metadata = catalog.metadata().expect("attested metadata");
        assert_eq!(metadata.platform_version, "8.3.27");
        assert_eq!(metadata.edt_version, "2026.1.2.2");
    }

    #[test]
    fn representative_symbols_are_bilingual_and_case_insensitive() {
        let catalog = PlatformGlobalCatalog::instance();
        for (ru, en, kind) in [
            ("СтрДлина", "StrLen", PlatformGlobalKind::Function),
            ("Метаданные", "Metadata", PlatformGlobalKind::Property),
            ("ГоризонтальноеПоложение", "HorizontalAlign", PlatformGlobalKind::SystemEnum),
            (
                "РасположениеПоляКомпоновкиДанных",
                "DataCompositionFieldPlacement",
                PlatformGlobalKind::SystemEnum,
            ),
        ] {
            let by_ru = catalog.lookup(ru).expect("Russian alias must resolve");
            let by_en =
                catalog.lookup(&en.to_ascii_uppercase()).expect("English alias must resolve");
            assert_eq!(by_ru.kind, kind);
            assert_eq!(by_en.kind, kind);
        }
    }

    #[test]
    fn exact_manifest_replaces_the_old_enum_heuristic() {
        let catalog = PlatformGlobalCatalog::instance();
        assert_eq!(
            catalog.lookup("БиблиотекаКартинок").map(|symbol| symbol.kind),
            Some(PlatformGlobalKind::Property),
            "global property wins the same-named system-enum compatibility entry"
        );
        assert!(catalog.lookup("HTTPМетод").is_none());
        assert_eq!(
            catalog.lookup("ВариантОткрытияОкна").map(|symbol| symbol.kind),
            Some(PlatformGlobalKind::SystemEnum)
        );
    }

    #[test]
    fn alias_collisions_have_compatible_use_capabilities() {
        let catalog = PlatformGlobalCatalog::instance();
        let mut aliases = FxHashMap::<SmolStr, PlatformSymbolCapabilities>::default();
        for symbol in catalog.symbols() {
            for alias in [&symbol.canonical_ru, &symbol.canonical_en]
                .into_iter()
                .filter(|alias| !alias.is_empty())
            {
                let folded = SmolStr::from(alias.as_str().fold_lower());
                if let Some(previous) = aliases.insert(folded.clone(), symbol.capabilities) {
                    assert_eq!(
                        previous.callable, symbol.capabilities.callable,
                        "alias {folded:?} mixes callable and non-callable symbols"
                    );
                    assert_eq!(
                        previous.readable_as_value, symbol.capabilities.readable_as_value,
                        "alias {folded:?} mixes readable and non-readable symbols"
                    );
                }
            }
        }
    }

    #[test]
    fn non_globals_and_issue_names_are_excluded() {
        let catalog = PlatformGlobalCatalog::instance();
        assert!(catalog.lookup("ТаблицаЗначений").is_none());
        assert!(catalog.lookup("CustomField").is_none());
        assert!(catalog.lookup("ГоризонтальноеПоложениеТабличногоДокумента").is_none());
        assert!(catalog.lookup("РасположениеПолейКомпоновкиДанных").is_none());
    }

    #[test]
    fn catalog_shape_matches_the_attested_edt_resources() {
        let catalog = PlatformGlobalCatalog::instance();
        let mut functions = 0;
        let mut properties = 0;
        let mut enums = 0;
        for symbol in catalog.symbols() {
            match symbol.kind {
                PlatformGlobalKind::Function => functions += 1,
                PlatformGlobalKind::Property => properties += 1,
                PlatformGlobalKind::SystemEnum => enums += 1,
            }
        }
        assert_eq!((functions, properties, enums), (507, 100, 628));
    }
}
