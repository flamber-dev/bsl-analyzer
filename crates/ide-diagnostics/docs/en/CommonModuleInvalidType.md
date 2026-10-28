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

These deviations are accepted, taken from the standard's conditional wording:

- a client-side module may set only one of the two client flags - a module
  available only in the managed or only in the ordinary application
  (std469, item 2.3);
- `Client (Ordinary application)` is required for server-side and
  client-server modules only when the configuration supports the ordinary
  application (`ordinaryAppSupport`), the same leniency the naming checks apply;
- `External connection` is optional for this type check on server-side and
  client-server modules: a module flagged only `Server` is an ordinary server
  module of a working configuration (std469, item 2.1 asks for the extra flags
  so the configuration works in those modes). Naming checks are not loosened:
  they still select the server and client-server templates that include
  `External connection`.

`Global`, `Privileged` and `Return values reuse` do not affect the result.
`Server call` is accepted only together with `Server` and no other context;
any other combination with `Server call`, and a module without any execution
context, is reported.


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
