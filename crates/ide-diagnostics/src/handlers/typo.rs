use crate::define_metadata;
use crate::metadata::*;
use crate::{BodyContext, Diagnostic, DiagnosticCode};
use hir::LocalRange;
use ide_db::TextRange;
use once_cell::sync::Lazy;
use std::collections::HashSet;
use std::sync::{Arc, RwLock};
use stdx::case::CaseExt;
use syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};
use text_size::TextSize;

pub const METADATA: DiagnosticMetadata = define_metadata! {
    diagnostic_type: DiagnosticType::CodeSmell,
    severity: DiagnosticSeverityLevel::Info,
    scope: DiagnosticScope::All,
    modules: &[],
    minutes_to_fix: 1,
    activated_by_default: false,
    compatibility_mode: DiagnosticCompatibilityMode::Undefined,
    tags: &[MetadataTag::Badpractice],
    can_locate_on_project: false,
    extra_min_for_complexity: 0.0,
    lsp_severity_override: "",
};

static DICTIONARIES: Lazy<RwLock<Option<Arc<Dictionaries>>>> = Lazy::new(|| RwLock::new(None));

const EN_AFF: &str = include_str!("../../dictionaries/en_US.aff");
const EN_DIC: &str = include_str!("../../dictionaries/en_US.dic");
const RU_AFF: &str = include_str!("../../dictionaries/ru_RU.aff");
const RU_DIC: &str = include_str!("../../dictionaries/ru_RU.dic");

const EN_EXCEPTIONS: &[&str] = &[
    "Str",
    "Autotest",
    "Infobase",
    "Enums",
    "Len",
    "Desc",
    "Asc",
    "Overridable",
    "GUID",
    "Extension",
    "Enum",
    "Storages",
    "MXL",
    "NFD",
    "IMAP",
    "smtp",
    "url",
    "Barcode",
    "img",
    "Crypto",
    "Decrypted",
    "Saa",
    "Dialogs",
    "xml",
    "Runtime",
    "Struct",
    "Pwd",
    "Decrypt",
    "Init",
    "Srvr",
    "Customizable",
    "Modally",
    "Eval",
    "http",
    "https",
    "Uncheck",
    "Wsdl",
    "Namespace",
    "Proc",
    "rac",
    "Substring",
    "Tmp",
    "Deserialization",
    "XSL",
    "Pos",
    "MMMM",
    "dddd",
    "Goto",
    "imap",
    "infobase",
    "Postfix",
    "Cryptographic",
    "mxl",
    "Extention",
    "DESC",
    "Sys",
    "Saas",
    "www",
    "yyyy",
    "xsl",
    "src",
    "deserialization",
    "Params",
    "Archiver",
    "Serializer",
    "xsi",
    "ico",
    "epf",
    "cfu",
    "txt",
    "htm",
    "rtf",
    "ppt",
    "vsd",
    "mpp",
    "mdb",
    "msg",
    "rar",
    "exe",
    "grs",
    "geo",
    "jpg",
    "bmp",
    "tif",
    "gif",
    "png",
    "pdf",
    "odt",
    "odf",
    "odp",
    "odg",
    "ods",
    "erf",
    "docx",
    "xlsx",
    "pptx",
    "utf",
    "xsd",
    "SRVR",
    "saas",
    "wsdl",
    "Apdex",
    "APDEX",
    "uid",
    "XLS",
    "XLSX",
    "html",
    "TXT",
    "ODT",
    "Addin",
    "DIB",
];

#[rustfmt::skip]
const RU_EXCEPTIONS: &[&str] = &[
    "Автогенерируемых", "Автогруппировку", "Автозаголовок", "Автоиспользование",
    "Автонастройка", "Автонастройку", "Автообновление", "Автоподбор",
    "Авторегистрация", "Автотест", "Бухфон", "Валидное", "Валидные",
    "Версионирование", "Версионирования", "Версионируемого", "Версионируемые",
    "Версионируемый", "Видеозвонки", "Возр", "Гант", "Ганта", "гггг", "Гипер",
    "Гиперссылкой", "Гиперссылку", "Гиперссылок", "Госрегулированием", "Грейд",
    "дд", "Декомпозировать", "Денормализовать", "Десериализация", "Десериализовать",
    "Дозаполнить", "Док", "Дт", "Журналирования", "Зарплатных", "Зацикленности",
    "Знч", "Исп", "Коннект", "Корсчета", "Кт", "Микропредприятий", "Многострочность",
    "Модифицированности", "Модифицированность", "Мульти", "Неинтерактивном",
    "Неиспользующихся", "Неодобренном", "Неопределен", "Неопределено", "Неполностью",
    "Непроведенный", "Непроведенных", "Непройденные", "Несинхронизируемые",
    "Несырьевых", "Однострочное", "Отсканированной", "Отсканированные", "Офшоре",
    "Офшоров", "Парсер", "Перезаполнить", "Перезаполнения", "Перезаполняемая",
    "Перезаполняется", "Перепроведение", "Перепроведением", "Перепроведения",
    "Повт", "Подзапросы", "Подотчетнику", "Подпапка", "Подпапки", "Подредакции",
    "Подредакция", "Процессинговых", "Псевдо", "Разыменователь", "Рег", "Регл",
    "Резидентство", "Сворачиваемости", "Сериализатор", "Сериализация",
    "Сериализованные", "Сериализованный", "Сис", "Сконвертировать", "Слеш",
    "Слеша", "Слеши", "Стартован", "Стикера", "Стр", "Студотряде", "Субконто",
    "Таб", "Техподдержки", "Токене", "Транслите", "Тэги", "Тэгов", "Убыв",
    "Физлица", "Финализировать", "Фич", "Хэш", "Штрихкодам", "Штрихкодом",
    "Штрихкоду", "Мдд", "Чммсс",
];

struct Dictionaries {
    ru: spellbook::Dictionary,
    en: spellbook::Dictionary,
}

#[derive(Debug, Clone)]
struct Config {
    min_word_length: usize,
    user_words_to_ignore: HashSet<String>,
    case_insensitive: bool,
}

impl Config {
    fn from_context(ctx: &BodyContext) -> Self {
        let code = DiagnosticCode::Typo;

        let min_word_length = ctx
            .config
            .get_int(code, "minWordLength")
            .and_then(|v| usize::try_from(v).ok())
            .unwrap_or(3);

        let user_words_raw = ctx.config.get_string(code, "userWordsToIgnore").unwrap_or("");

        let user_words_to_ignore = user_words_raw
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();

        let case_insensitive = ctx.config.get_bool(code, "caseInsensitive").unwrap_or(false);

        Self { min_word_length, user_words_to_ignore, case_insensitive }
    }

    fn should_ignore(&self, word: &str) -> bool {
        if self.case_insensitive {
            let word_lower = word.fold_lower();
            self.user_words_to_ignore.iter().any(|w| w.fold_lower() == word_lower)
        } else {
            self.user_words_to_ignore.contains(word)
        }
    }
}

fn get_dictionaries() -> Arc<Dictionaries> {
    {
        let guard = DICTIONARIES.read().unwrap();
        if let Some(dict) = guard.as_ref() {
            return Arc::clone(dict);
        }
    }

    let mut guard = DICTIONARIES.write().unwrap();
    if let Some(dict) = guard.as_ref() {
        return Arc::clone(dict);
    }

    let ru = spellbook::Dictionary::new(RU_AFF, RU_DIC)
        .expect("Failed to load Russian Hunspell dictionary");
    let en = spellbook::Dictionary::new(EN_AFF, EN_DIC)
        .expect("Failed to load English Hunspell dictionary");

    let dicts = Arc::new(Dictionaries { ru, en });
    *guard = Some(Arc::clone(&dicts));
    dicts
}

fn split_camel_case(text: &str) -> Vec<(String, usize)> {
    let mut words = Vec::new();
    let mut current_word = String::new();
    let mut start_offset = 0;
    let chars: Vec<char> = text.chars().collect();

    for (i, &ch) in chars.iter().enumerate() {
        if ch.is_uppercase() && !current_word.is_empty() {
            words.push((current_word.clone(), start_offset));
            current_word.clear();
            start_offset = i;
        }
        current_word.push(ch);
    }

    if !current_word.is_empty() {
        words.push((current_word, start_offset));
    }

    words
}

fn is_format_string(text: &str) -> bool {
    text.contains("ДФ=")
        || text.contains("L=")
        || text.contains("ND=")
        || text.contains("NZ=")
        || text.contains("NG=")
}

fn is_valid_word(word: &str, dictionaries: &Dictionaries) -> bool {
    if EN_EXCEPTIONS.contains(&word) || RU_EXCEPTIONS.contains(&word) {
        return true;
    }

    dictionaries.en.check(word) || dictionaries.ru.check(word)
}

fn remove_quotes(text: &str) -> String {
    if text.len() < 2 {
        return text.to_string();
    }
    let first = text.chars().next();
    let last = text.chars().last();
    if first == Some('"') && last == Some('"') || first == Some('\'') && last == Some('\'') {
        text[1..text.len() - 1].to_string()
    } else {
        text.to_string()
    }
}

fn check_ident_token(
    token: &SyntaxToken,
    acc: &mut Vec<Diagnostic<LocalRange>>,
    ctx: &BodyContext,
    config: &Config,
    dictionaries: &Dictionaries,
) {
    let code = DiagnosticCode::Typo;
    let text = token.text().to_string();
    let words = split_camel_case(&text);

    for (word, offset) in words {
        if word.len() < config.min_word_length {
            continue;
        }

        if config.should_ignore(&word) {
            continue;
        }

        if !is_valid_word(&word, dictionaries) {
            let start: u32 = token.text_range().start().into();
            let word_start = start + offset as u32;
            let word_end = word_start + word.len() as u32;

            acc.push(Diagnostic {
                code,
                message: format!("Возможная опечатка в \"{}\"", word),
                severity: ctx.severity(code),
                range: LocalRange::of_detached_node(TextRange::new(
                    word_start.into(),
                    word_end.into(),
                )),
                tags: ctx.tags(code).to_vec(),
                fixes: vec![],
            });
        }
    }
}

fn find_first_ident_token(node: &SyntaxNode) -> Option<SyntaxToken> {
    node.children_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|token| token.kind() == SyntaxKind::IDENT)
}

fn find_lvalue_ident_token(node: &SyntaxNode) -> Option<SyntaxToken> {
    for element in node.children_with_tokens() {
        if let SyntaxElement::Token(token) = element {
            if token.kind() == SyntaxKind::IDENT {
                return Some(token);
            }
            if token.kind() == SyntaxKind::EQ {
                return None;
            }
        }
    }
    None
}

pub fn check_node(node: &SyntaxNode, acc: &mut Vec<Diagnostic<LocalRange>>, ctx: &BodyContext) {
    let code = DiagnosticCode::Typo;

    if ctx.is_disabled_with_metadata(code) {
        return;
    }

    let config = Config::from_context(ctx);
    let dictionaries = get_dictionaries();

    match node.kind() {
        SyntaxKind::PROCEDURE_DEF | SyntaxKind::FUNCTION_DEF => {
            if let Some(token) = find_first_ident_token(node) {
                check_ident_token(&token, acc, ctx, &config, &dictionaries);
            }
        }
        SyntaxKind::VAR_DEF => {
            if let Some(token) = find_first_ident_token(node) {
                check_ident_token(&token, acc, ctx, &config, &dictionaries);
            }
        }
        SyntaxKind::PARAM => {
            if let Some(token) = find_first_ident_token(node) {
                check_ident_token(&token, acc, ctx, &config, &dictionaries);
            }
        }
        SyntaxKind::ASSIGN_STMT => {
            if let Some(token) = find_lvalue_ident_token(node) {
                check_ident_token(&token, acc, ctx, &config, &dictionaries);
            }
        }
        _ => {
            if let Some(token) = node.first_token() {
                if token.kind() == SyntaxKind::STRING {
                    let text = remove_quotes(token.text());
                    if !is_format_string(&text) {
                        check_text(
                            &text,
                            token.text_range().start(),
                            acc,
                            ctx,
                            &config,
                            &dictionaries,
                        );
                    }
                }
            }
        }
    }
}

fn check_text(
    text: &str,
    base_offset: TextSize,
    acc: &mut Vec<Diagnostic<LocalRange>>,
    ctx: &BodyContext,
    config: &Config,
    dictionaries: &Dictionaries,
) {
    let code = DiagnosticCode::Typo;

    for word in text.split_whitespace() {
        let word = word.trim_matches(|c: char| !c.is_alphabetic());

        if word.len() < config.min_word_length {
            continue;
        }

        if config.should_ignore(word) {
            continue;
        }

        if !is_valid_word(word, dictionaries) {
            if let Some(pos) = text.find(word) {
                let start: u32 = base_offset.into();
                let word_start = start + pos as u32;
                let word_end = word_start + word.len() as u32;

                acc.push(Diagnostic {
                    code,
                    message: format!("Возможная опечатка в \"{}\"", word),
                    severity: ctx.severity(code),
                    range: LocalRange::of_detached_node(TextRange::new(
                        word_start.into(),
                        word_end.into(),
                    )),
                    tags: ctx.tags(code).to_vec(),
                    fixes: vec![],
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::test_utils::check_hir_diagnostic_with_config;
    use crate::{DiagnosticCode, DiagnosticsConfig};
    use expect_test::expect;

    const CLEAN: &str = r#"Функция ОстатокТеста(Замес)
	Сообщить("Тесто готово");
	Возврат Замес.Количество;
КонецФункции

Процедура ЗаполнитьЦены()
	Цена = Формат(ДатаВыпечки, "ДФ=дд.ММ.гг");
	ЗагрузитьДанныеЕГАИСОстатков = Ложь;
	ТипЗнч(Цена);
КонецПроцедуры
"#;

    const FIXTURE: &str = r#"Функция ОстатокТеста(Замес)
	Сообщить("Тесто гатово");
	Возврат Замес.Количество;
КонецФункции

Процедура ЗаполнитьЦеены()
	Цена = Формат(ДатаВыпечки, "ДФ=дд.ММ.гг");
	ЗагрузитьДанныеЕГАИСОстатков = Ложь;
	Сообщить("zqx");
КонецПроцедуры
"#;

    fn config_with_typo_enabled() -> DiagnosticsConfig {
        let mut config = DiagnosticsConfig::default();
        config.enabled.push(DiagnosticCode::Typo);
        config
    }

    /// The distinct words reported as typos, one per line, sorted.
    ///
    /// Words, not ranges: today each word is reported several times and its range is
    /// shifted by the opening quote of the literal (recorded as a finding in the
    /// attestation), so a range snapshot would pin that defect instead of the rule.
    fn snapshot(code: &str, config: DiagnosticsConfig, expected: expect_test::Expect) {
        let all = check_hir_diagnostic_with_config(code, config, crate::diagnostics);
        let mut words: Vec<_> =
            all.into_iter().filter(|d| d.code == DiagnosticCode::Typo).map(|d| d.message).collect();
        words.sort();
        words.dedup();
        expected.assert_eq(&words.join("\n"));
    }

    #[test]
    fn test_disabled_by_default() {
        snapshot(FIXTURE, DiagnosticsConfig::default(), expect![[r#""#]]);
    }

    #[test]
    fn test_dictionary_words_are_silent() {
        snapshot(CLEAN, config_with_typo_enabled(), expect![[r#""#]]);
    }

    #[test]
    fn test_typo_basic() {
        snapshot(
            FIXTURE,
            config_with_typo_enabled(),
            expect![[r#"
            Возможная опечатка в "zqx"
            Возможная опечатка в "Цеены"
            Возможная опечатка в "гатово""#]],
        );
    }

    #[test]
    fn test_typo_with_min_word_length() {
        let mut config = config_with_typo_enabled();
        config.parameters.insert(DiagnosticCode::Typo, serde_json::json!({"minWordLength": 4}));
        snapshot(
            FIXTURE,
            config,
            expect![[r#"
            Возможная опечатка в "Цеены"
            Возможная опечатка в "гатово""#]],
        );
    }

    #[test]
    fn test_typo_with_user_words_to_ignore() {
        let mut config = config_with_typo_enabled();
        config
            .parameters
            .insert(DiagnosticCode::Typo, serde_json::json!({"userWordsToIgnore": "Цеены"}));
        snapshot(
            FIXTURE,
            config,
            expect![[r#"
            Возможная опечатка в "zqx"
            Возможная опечатка в "гатово""#]],
        );
    }

    #[test]
    fn test_typo_case_insensitive() {
        let mut config = config_with_typo_enabled();
        config.parameters.insert(
            DiagnosticCode::Typo,
            serde_json::json!({"userWordsToIgnore": "цЕЕНЫ", "caseInsensitive": true}),
        );
        snapshot(
            FIXTURE,
            config,
            expect![[r#"
            Возможная опечатка в "zqx"
            Возможная опечатка в "гатово""#]],
        );
    }

    #[test]
    fn test_ignore_list_is_case_sensitive_by_default() {
        let mut config = config_with_typo_enabled();
        config
            .parameters
            .insert(DiagnosticCode::Typo, serde_json::json!({"userWordsToIgnore": "цЕЕНЫ"}));
        snapshot(
            FIXTURE,
            config,
            expect![[r#"
            Возможная опечатка в "zqx"
            Возможная опечатка в "Цеены"
            Возможная опечатка в "гатово""#]],
        );
    }

    #[test]
    fn test_dictionary_common_bsl_words() {
        use super::{get_dictionaries, is_valid_word};
        let dictionaries = get_dictionaries();

        let should_be_valid = [
            "Пока",
            "Цикл",
            "Количество",
            "Сумма",
            "Дата",
            "Ошибка",
            "Сообщение",
            "Создает",
            "Удаляет",
            "Записывает",
            "последний",
            "следующий",
            "пустой",
            "общий",
            "Value",
            "Count",
        ];

        for word in should_be_valid {
            assert!(
                is_valid_word(word, &dictionaries),
                "Word should be valid but Hunspell rejects: {}",
                word
            );
        }
        assert!(!is_valid_word("Цеены", &dictionaries), "мерка обязана отвергать опечатку");
    }
}
