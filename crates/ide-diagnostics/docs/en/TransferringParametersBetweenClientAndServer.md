# Transferring parameters between the client and the server (TransferringParametersBetweenClientAndServer)

<!-- Блоки выше заполняются автоматически, не трогать -->
## Description

When control is transferred from the client to the server, parameters are copied. If a server method parameter is declared without `Val` / `Знач`, the value may also be copied back to the client when control returns.

If such a parameter is never reassigned inside the server method, this reverse transfer is usually unnecessary and creates extra traffic.

The current implementation reports server methods when all of these conditions hold:

- the method has `&AtServer` or `&AtServerNoContext`;
- it is directly called from an `&AtClient` method in the same module;
- the parameter is not marked with `Val`;
- the parameter is not assigned inside the server method body.

If the server method changes the parameter in place - assigns a field or an element (`P.X = ...`, `P[...] = ...`, also deeper `P.X.Y = ...`) or calls a mutating collection method on it or on its field (`Add`/`Добавить`, `Insert`/`Вставить`, `Delete`/`Удалить`, `Clear`/`Очистить`, `Set`/`Установить`, `Load`/`Загрузить`, `LoadValues`/`ЗагрузитьЗначения`, `LoadColumn`/`ЗагрузитьКолонку`, `FillValues`/`ЗаполнитьЗначения`, `FillChecks`/`ЗаполнитьПометки`, `GroupBy`/`Свернуть`, `Sort`/`Сортировать`, `SortByValue`/`СортироватьПоЗначению`, `SortByPresentation`/`СортироватьПоПредставлению`, `Move`/`Сдвинуть`). The change is also recognized through a method call in the chain (`P.Rows.Find(1).Sum = ...`), through a local alias (`X = P.Goods; X.Clear()`), through a `For Each` variable (`For Each Row In P.Goods Do Row.Sum = 0`) when `FillPropertyValues`/`ЗаполнитьЗначенияСвойств` fills it, and when the parameter is passed to a method of the same module that changes it. The parameter is also followed when it is stored in a structure or collection (`L.Goods = P`, `New Structure("Data", P)`), returned from a function of the module (`T = GetGoods(P)`) or kept in a module variable that another method of the module changes; an alias stops counting once it is assigned another value; a copy (`Copy`/`Скопировать`, `Unload`/`Выгрузить`, `UnloadValues`/`ВыгрузитьЗначения`, `UnloadColumn`/`ВыгрузитьКолонку`, `CopyColumns`/`СкопироватьКолонки`) is not the parameter's data. In all these cases the caller reads that data after the call, and `Val` would silently drop it. Such a parameter is still reported (the standard does not recommend returning values from the server through parameters), but with a different message - return the data from a function instead of changing the parameter - and without the `Val` quick fix.

## Examples 
<!-- This section contains examples for which the diagnostics work, and you can also give an example of how to fix the situation -->

```bsl
&AtClient
Procedure ShowBalance(Command)
    Result = GetBalance(CurrentItem, CurrentWarehouse);
EndProcedure

&AtServerNoContext
Function GetBalance(Item, Warehouse)
    Return AccumulationRegisters.Stock.GetBalance(Item, Warehouse);
EndFunction
```

Recommended:

```bsl
&AtClient
Procedure ShowBalance(Command)
    Result = GetBalance(CurrentItem, CurrentWarehouse);
EndProcedure

&AtServerNoContext
Function GetBalance(Val Item, Val Warehouse)
    Return AccumulationRegisters.Stock.GetBalance(Item, Warehouse);
EndFunction
```

## Sources 

- [1C ITS article: call with transfer of control from client to server (RU)](https://its.1c.ru/db/v8318doc#bookmark:dev:TI000000153)
- [#std487: Minimizing server calls and traffic (RU)](https://its.1c.ru/db/v8std#content:487:hdoc)
