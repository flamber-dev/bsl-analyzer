# Unknown type in a constructor (UnknownTypeInNew)

## Description

`New <Name>` names a type the platform does not have. This is a compile error, not a
run-time one: the platform refuses to compile the whole module with «Тип не определен»
("Type is not defined"), so every procedure of the module becomes unavailable, not just
the line with the typo.

The diagnostic fires on the form `New <Identifier>` (with or without arguments) when the
name is neither a platform type (Russian and English spellings, case-insensitive) nor a
type family the analyzer knows the platform serves outside its type list.

## When the diagnostic is silent

- the type name is a value: `New("Array")`, `New(TypeName)`, `New(Type("..."))` — the
  name is a value known only at run time;
- the platform help is not served in full (no help, or the requested source degraded to
  the built-in facts), or the project's target platform is newer than the served help:
  a newer release may add types the list lacks;
- the call sits in an `#If` branch no environment of the method compiles, or in a common
  module with no environments set;
- the name belongs to a family the platform serves outside its type list: form controls,
  managed-form extensions, XDTO/XML/DOM types, metadata-object descriptions, register
  record sets and keys;
- a pre-rename type name the platform still compiles but the help lists only under
  the new name (a short fixed list, currently `БлокировкаУстановкиСоединений`, the old
  name of `БлокировкаСеансов`); the list is not exhaustive;
- a type that exists but is not available in the current execution environment is
  `UnavailableInEnvironment`, not this diagnostic.

## Examples

Incorrect:

```bsl
Result = New NoSuchObjectType;
```

Correct:

```bsl
Result = New Array;
Result = New("Array");
```
