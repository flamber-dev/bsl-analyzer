# Platform member hidden by the compatibility mode (PlatformMemberHiddenByCompatibilityMode)

## Description

A configuration's compatibility mode (`<CompatibilityMode>` in `Configuration.xml`)
hides part of the global context from code, however new the platform is. The
analyzer checks code against the 8.3.27 catalog and ignores the mode, so `StrSplit`
in a configuration with mode 8.2.13 passes silently while the module does not
compile at the customer: "Процедура или функция с указанным именем не определена
(СтрРазделить)".

What a mode hides was measured, not taken from the help: on 8.3.27.2214 and
8.3.17.1549 all 604 global functions and properties were compiled on bases that
differ only in the compatibility mode (8.2.13, 8.3.1 … 8.3.19, 8.3.27). Below their
threshold exactly 30 names (and their English aliases) are hidden; everything else
— other new functions, all types, methods, `…Async` — is visible even in mode 8.2.13.

| Visible from mode | Names |
|---|---|
| 8.3.3 | `DynamicListsUserSettingsStorage` (property) |
| 8.3.6 | `StrFind`, `StrSplit`, `StrConcat`, `StrTemplate`, `StrCompare`, `StrStartsWith`, `StrEndsWith`, `GetUUIDWithCompatibilitySupport` |
| 8.3.9 | `GetSafeModeDisabled`, `SetSafeModeDisabled`, `ProceedWithCall`, `GetTotalRecalcJobCount`, `SetTotalRecalcJobCount`, `StrReplaceByRegularExpression`, `StrFindByRegularExpression`, `StrFindAllByRegularExpression`, `StrLikeByRegularExpression` |
| 8.3.11 | `BitwiseAnd`, `BitwiseOr`, `BitwiseNot`, `BitwiseAndNot`, `BitwiseXor`, `BitwiseShiftLeft`, `BitwiseShiftRight`, `CheckBit`, `CheckByBitMask`, `SetBit` |
| 8.3.14 | `DatabaseCopies` (property); the property `Query.RequiredDataRelevance` |
| 8.3.19 | `URLExternalDataStorage` (property) |

A global name below its threshold breaks compilation of the whole module. The
property `Query.RequiredDataRelevance` is late-bound: the module compiles and the
line fails when executed — "Поле объекта не обнаружено".

The mode comes from the project setting `compatibility_mode`, else from
`<CompatibilityMode>` of the main configuration (see
`docs/configuration/PROJECT_CONFIGURATION.md`). It applies to the modules of the
configuration, of its extensions (an extension has no mode of its own — it runs in
the mode of the configuration it extends) and of the project's external data
processors and reports: an external processor compiles in the mode of the base it is
opened in (checked live). The setting is for sources that do not match the
customer's base: the export says 8.2.13 while the customer runs 8.3.15, or the other
way round.

## When the diagnostic stays silent

- the mode is unknown: no setting and no main configuration `Configuration.xml` (a
  project of a single extension or of external processors only);
- the mode is `DontUse` or a value that is not a mode;
- the mode is not below the name's threshold;
- a module method or an export of a global common module holds the name — the way a
  replacement for an older mode is written;
- the value's type is not inferred as `Query`;
- the call sits in an `#If` branch no environment of the method compiles, or in a
  common module with no environment ticked.

This rule is not about the platform release: a function the older platform lacks is
caught by `PlatformMemberNewerThanMinVersion`. For instance,
`StrReplaceByRegularExpression` in mode 8.3.17 is not reported here (visible from
mode 8.3.9), while platform 8.3.17 does not have it (since 8.3.23).

## Examples

The configuration's compatibility mode is 8.2.13.

Incorrect:

```bsl
Procedure Parse(Text)
    Parts = StrSplit(Text, ","); // visible from mode 8.3.6
EndProcedure
```

Correct:

```bsl
Function SplitString(Val Text, Separator)
    Parts = New Array;
    Position = Find(Text, Separator);
    While Position > 0 Do
        Parts.Add(Left(Text, Position - 1));
        Text = Mid(Text, Position + StrLen(Separator));
        Position = Find(Text, Separator);
    EndDo;
    Parts.Add(Text);
    Return Parts;
EndFunction
```
