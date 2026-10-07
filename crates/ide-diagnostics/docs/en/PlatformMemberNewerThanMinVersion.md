# Platform member newer than the project's minimum platform (PlatformMemberNewerThanMinVersion)

## Description

The diagnostic is for projects developed on one platform release and run in
production on an older one (say, development on 8.3.27, production on 8.3.17). The
analyzer checks code against a single bundled platform catalog, so a function the
older platform does not have yet passes silently — and the module stops compiling
only at the customer: "Процедура или функция с указанным именем не определена".

The configuration's 8.3.x compatibility mode does not help: checked live on
8.3.17.1549 and 8.3.27.2214, 8.3.x modes do not hide newer platform members — what
exists is decided by the platform release the code runs on.

The rule is switched on by the project setting `min_platform_version` (see
`docs/configuration/PROJECT_CONFIGURATION.md`). While the setting is unset, the rule
is completely silent. Reported is every use of a platform member whose catalog
"available since" version is later than the configured one:

- a global function or global property called or read by its bare name (unless a
  module method, a variable or an export of a global common module holds the name);
- the type in `Новый Type(...)` (and `Новый("Type")`): the type's date, or, when
  every constructor of it is dated and the oldest is later than the type, that
  constructor's date;
- a method or property of a value the analyzer typed as a platform type; the later
  of the member's and its type's dates applies;
- a method of a global property (`ХранилищеДвоичныхДанных.Создать()`);
- an `Асинх` declaration and the `Ждать` operator — from 8.3.18 (8.3.17.1549 rejects
  an async procedure: "Неопознанный оператор").

The consequences differ. A call of a global function the platform lacks, and
`Асинх`, break compilation of the whole module (checked on 8.3.17). A method or
property of a value is bound late: the module compiles, and executing that line
fails.

## When the diagnostic is silent

- `min_platform_version` is unset or is not a version number;
- the member has no version in the catalog (or it cannot be parsed) — missing data
  is never read as "new";
- the member's catalog date is contradicted by working code: currently
  `ХешированиеДанных.ХешСумма` (dated 8.3.18, read by the BSP on 8.3.17);
- the value's type is not inferred, or not as exactly one platform type
  (configuration objects, unions);
- the use sits in a `#Если` branch no environment of the method compiles, or in a
  common module with no environment selected.

`target_platform_version` does not affect this rule, nor the rule it.

## Examples

The project's minimum platform is 8.3.17.

Incorrect:

```bsl
Процедура Очистить(Текст)
    Текст = СтрЗаменитьПоРегулярномуВыражению(Текст, "\s+", " "); // since 8.3.23
КонецПроцедуры

Асинх Процедура Показать() // since 8.3.18
    Ждать ПредупреждениеАсинх("Готово");
КонецПроцедуры
```

Correct:

```bsl
Процедура Очистить(Текст)
    Части = СтрРазделить(Текст, " ", Ложь); // since 8.3.6
    Текст = СтрСоединить(Части, " ");
КонецПроцедуры

Процедура Показать()
    ПоказатьПредупреждение(, "Готово");
КонецПроцедуры
```
