# Call to an unresolved method (UnresolvedMethodCall)

<!-- Блоки выше заполняются автоматически, не трогать -->
## Description

The diagnostic reports a qualified method call that cannot be resolved confidently by the analyzer.

This usually means one of the following:

- the common module name is wrong;
- the method name contains a typo;
- the method exists but is not exported;
- metadata refers to a common module whose source file is missing from the workspace.

The current implementation is conservative and focuses on qualified calls such as `CommonModule.Method()`, where the resolver has enough information to produce a useful error.

## Examples

Incorrect:

```bsl
CommonModule.UnknownMethod();
```

Correct:

```bsl
CommonModule.KnownMethod();
```

## Calls through ThisObject / ThisForm in a form module

Only the exported methods of a form module are reachable through `ThisObject` and `ThisForm`. A non-exported method does not break the module, but the call fails at run time with "Object method not found" (checked on 8.3.17 and 8.3.27, on the client and on the server).

Incorrect:

```bsl
&AtClient
Procedure Show()
EndProcedure

&AtClient
Procedure Save()
    ThisForm.Show();
EndProcedure
```

Correct:

```bsl
&AtClient
Procedure Save()
    Show();
EndProcedure
```

or declare the method `Export`.
