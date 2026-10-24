# Common module invalid type (CommonModuleInvalidType)

<!-- Блоки выше заполняются автоматически, не трогать -->
## Description

Common modules should use one of the standard execution-context combinations
described in the 1C development rules. Arbitrary flag combinations make the
module purpose harder to understand and can produce confusing behavior when the
same module is called from different runtime contexts.

This diagnostic reports common modules whose metadata flags do not match one of
the accepted module types.

| Common module type              | Naming example                 | Server call | Server | External connection | Client (Ordinary application) | Client (Managed application) |
| ------------------------------- | ------------------------------ | ----------- | ------ | ------------------- | ----------------------------- | ---------------------------- |
| Server-side                     | Common (or CommonServer)       |             | +      | +                   | +                             |                              |
| Server-side to call from client | CommonServerCall               | +           | +      |                     |                               |                              |
| Client-side                     | CommonClient (or CommonGlobal) |             |        |                     | +                             | +                            |
| Client-server                   | CommonClientServer             |             | +      | +                   | +                             | +                            |

Two deviations are accepted, both taken from the standard:

- a client-side module may set only one of the two client flags - a module
  available only in the managed or only in the ordinary application
  (std469, item 2.3);
- `Client (Ordinary application)` is required for server-side and
  client-server modules only when the configuration supports the ordinary
  application (`ordinaryAppSupport`), the same leniency the naming checks apply.

`Global`, `Privileged` and `Return values reuse` do not affect the result. Any
combination of `Server call` with other flags, and a module without any
execution context, is reported.


## Examples

### Wrong

A common module combines flags in a way that does not correspond to `Server`,
`ServerCall`, `Client`, or `ClientServer`.

### Correct

Choose one of the standard module types and align both the metadata flags and
the module naming convention with that type.

## Sources

Primary source: [Standard: rules for creating common modules (RU)](https://its.1c.ru/db/v8std#content:469:hdoc:1.2)

Secondary source: [v8std.ru: #std469](https://v8std.ru/std/469/)

Additional reference: [v8std.ru: CommonModuleInvalidType](https://v8std.ru/diagnostics/bslls/CommonModuleInvalidType/)
