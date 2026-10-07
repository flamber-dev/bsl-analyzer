//! The configuration's compatibility mode reaches `analyze` — from
//! `Configuration.xml` and from the `compatibility_mode` setting — and governs the
//! external data processors of the project too: the platform compiles an external
//! processor in the mode of the base it is opened in.

use std::path::Path;
use std::process::Command;

use serde_json::Value;

const CODE: &str = "PlatformMemberHiddenByCompatibilityMode";
const CALL: &str = "Части = СтрРазделить(\"a,b\", \",\");";

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// `src/cf` in the given mode with one server common module, plus an external
/// processor `src/epf/Обработка` whose form module makes the same call.
fn project(root: &Path, mode: &str, setting: Option<&str>) {
    let cf = root.join("src/cf");
    write(
        &cf.join("Configuration.xml"),
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:v8="http://v8.1c.ru/8.1/data/core">
	<Configuration uuid="11111111-0000-0000-0000-000000000001">
		<Properties><Name>Конфигурация</Name><Synonym/><Comment/><DefaultRunMode>ManagedApplication</DefaultRunMode><CompatibilityMode>{mode}</CompatibilityMode></Properties>
		<ChildObjects><CommonModule>Модуль</CommonModule></ChildObjects>
	</Configuration>
</MetaDataObject>"#
        ),
    );
    write(
        &cf.join("CommonModules/Модуль.xml"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:v8="http://v8.1c.ru/8.1/data/core">
	<CommonModule uuid="22222222-0000-0000-0000-000000000002">
		<Properties><Name>Модуль</Name><Synonym/><Comment/><Global>false</Global><ClientManagedApplication>false</ClientManagedApplication><Server>true</Server><ExternalConnection>false</ExternalConnection><ClientOrdinaryApplication>false</ClientOrdinaryApplication><ServerCall>false</ServerCall><Privileged>false</Privileged><ReturnValuesReuse>DontUse</ReturnValuesReuse></Properties>
	</CommonModule>
</MetaDataObject>"#,
    );
    write(
        &cf.join("CommonModules/Модуль/Ext/Module.bsl"),
        &format!("Процедура Проверить() Экспорт\n\t{CALL}\nКонецПроцедуры\n"),
    );

    let epf = root.join("src/epf/Обработка");
    write(
        &epf.join("Обработка.xml"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">
	<ExternalDataProcessor uuid="3696c164-ad14-4a0d-b659-10e3bf6d6ad2">
		<Properties><Name>Обработка</Name><Synonym/><Comment/><DefaultForm>ExternalDataProcessor.Обработка.Form.Форма</DefaultForm></Properties>
		<ChildObjects><Form>Форма</Form></ChildObjects>
	</ExternalDataProcessor>
</MetaDataObject>"#,
    );
    write(
        &epf.join("Обработка/Forms/Форма.xml"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">
	<Form uuid="8919791a-5b27-410f-9404-010ce96c6db6">
		<Properties><Name>Форма</Name><Synonym/><Comment/><FormType>Managed</FormType></Properties>
	</Form>
</MetaDataObject>"#,
    );
    write(
        &epf.join("Обработка/Forms/Форма/Ext/Form.xml"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">
	<AutoCommandBar name="ФормаКоманднаяПанель" id="-1"/>
	<Attributes/>
</Form>"#,
    );
    write(
        &epf.join("Обработка/Forms/Форма/Ext/Form/Module.bsl"),
        &format!("&НаСервере\nПроцедура Проверить()\n\t{CALL}\nКонецПроцедуры\n"),
    );

    let setting =
        setting.map_or(String::new(), |mode| format!("compatibility_mode = \"{mode}\"\n"));
    write(
        &root.join("bsl-analyzer.toml"),
        &format!(
            "{setting}[source]\nroot = \"src/cf\"\nextensions = []\nexternals = [{{ name = \"Обработка\", path = \"src/epf/Обработка\" }}]\n"
        ),
    );
}

/// Messages of [`CODE`] per module, keyed by the module's path tail.
fn analyze(root: &Path) -> (Vec<String>, Vec<String>) {
    let output = Command::new(env!("CARGO_BIN_EXE_bsl-analyzer-app"))
        .arg("analyze")
        .arg("-s")
        .arg(root)
        .args(["--format", "jsonl"])
        .env_remove("ONEC_CONFIGURATIONS_ROOT")
        .output()
        .expect("failed to run the analyzer");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let files: Vec<Value> = stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .filter(|e| e["type"] == "file")
        .collect();
    let messages = |tail: &str| -> Vec<String> {
        let event = files
            .iter()
            .find(|e| e["path"].as_str().is_some_and(|p| Path::new(p).ends_with(tail)))
            .unwrap_or_else(|| panic!("{tail} was not analyzed; files: {files:?}"));
        assert_eq!(event["error"], Value::Null, "{event}");
        event["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["code"].as_str() == Some(CODE))
            .map(|d| d["message"].as_str().unwrap_or_default().to_owned())
            .collect()
    };
    (
        messages("CommonModules/Модуль/Ext/Module.bsl"),
        messages("Обработка/Forms/Форма/Ext/Form/Module.bsl"),
    )
}

#[test]
fn an_8_2_13_configuration_hides_str_split_in_its_modules_and_its_externals() {
    let dir = tempfile::tempdir().unwrap();
    project(dir.path(), "Version8_2_13", None);
    let (configuration, external) = analyze(dir.path());
    assert_eq!(configuration.len(), 1, "{configuration:?}");
    assert!(configuration[0].contains("'СтрРазделить'"), "{configuration:?}");
    assert!(configuration[0].contains("8.2.13"), "{configuration:?}");
    assert_eq!(external.len(), 1, "the external runs in the base's mode: {external:?}");
    assert!(external[0].contains("'СтрРазделить'"), "{external:?}");
}

#[test]
fn a_later_mode_and_dont_use_are_silent() {
    for mode in ["Version8_3_17", "DontUse"] {
        let dir = tempfile::tempdir().unwrap();
        project(dir.path(), mode, None);
        let (configuration, external) = analyze(dir.path());
        assert!(
            configuration.is_empty() && external.is_empty(),
            "{mode}: {configuration:?} {external:?}"
        );
    }
}

#[test]
fn the_setting_overrides_configuration_xml() {
    let dir = tempfile::tempdir().unwrap();
    project(dir.path(), "Version8_2_13", Some("8.3.15"));
    let (configuration, external) = analyze(dir.path());
    assert!(configuration.is_empty() && external.is_empty(), "{configuration:?} {external:?}");

    let dir = tempfile::tempdir().unwrap();
    project(dir.path(), "Version8_3_17", Some("Version8_2_13"));
    let (configuration, external) = analyze(dir.path());
    assert_eq!((configuration.len(), external.len()), (1, 1), "{configuration:?} {external:?}");
}
