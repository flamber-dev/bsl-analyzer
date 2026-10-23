use crate::define_metadata;
use crate::metadata::*;
use crate::{Diagnostic, DiagnosticCode, DiagnosticsContext, Fix, TextEdit};
use hir::call_graph::{CallTarget, CallerId, EdgeKind};
use hir::{AnnotationKind, Expr, IdConversion, Stmt};
use ide_db::TextRange;
use rustc_hash::{FxHashMap, FxHashSet};
use stdx::case::CaseExt;

pub const METADATA: DiagnosticMetadata = define_metadata! {
    diagnostic_type: DiagnosticType::CodeSmell,
    severity: DiagnosticSeverityLevel::Major,
    scope: DiagnosticScope::Bsl,
    modules: &[],
    minutes_to_fix: 2,
    activated_by_default: true,
    compatibility_mode: DiagnosticCompatibilityMode::Undefined,
    tags: &[MetadataTag::Badpractice, MetadataTag::Performance, MetadataTag::Standard],
    can_locate_on_project: false,
    extra_min_for_complexity: 0.0,
    lsp_severity_override: "",
    clean_code_attribute: CleanCodeAttribute::Adaptable,
};

const SERVER_ANNOTATIONS: &[AnnotationKind] =
    &[AnnotationKind::AtServer, AnnotationKind::AtServerNoContext];

const CLIENT_ANNOTATION: AnnotationKind = AnnotationKind::AtClient;

pub fn check(ctx: &DiagnosticsContext) -> Vec<Diagnostic> {
    let code = DiagnosticCode::TransferringParametersBetweenClientAndServer;

    if ctx.is_disabled_with_metadata(code) {
        return Vec::new();
    }

    let symbol_tree = ctx.symbol_tree();
    let module_bodies = ctx.module_bodies();
    let summary = ctx.call_summary(hir::ModuleId::new(ctx.file_id));
    let mut mutation_scan = MutationScan::new(&module_bodies, &symbol_tree);
    let mut diagnostics = Vec::new();

    let client_method_ids: FxHashSet<hir::MethodKey> = symbol_tree
        .methods()
        .filter(|m| m.annotations.iter().any(|ann| ann.kind == CLIENT_ANNOTATION))
        .map(|m| m.id.local_id)
        .collect();

    for method in symbol_tree.methods() {
        let has_server_annotation =
            method.annotations.iter().any(|ann| SERVER_ANNOTATIONS.contains(&ann.kind));

        if !has_server_annotation {
            continue;
        }

        let by_ref_params: Vec<_> =
            method.params.iter().enumerate().filter(|(_, param)| !param.is_val).collect();

        if by_ref_params.is_empty() {
            continue;
        }

        let server_local_id = method.id.local_id;
        let has_client_call = summary.call_edges.iter().any(|edge| {
            edge.kind == EdgeKind::DirectLocal
                && matches!(&edge.target, CallTarget::Local { callee_local_id } if *callee_local_id == server_local_id)
                && matches!(edge.caller, CallerId::Method(caller_id) if client_method_ids.contains(&caller_id))
        });

        if !has_client_call {
            continue;
        }

        let local_id = method.id.local_id;
        let Some(lower_result) = module_bodies.lower_result(local_id) else {
            continue;
        };

        let body = lower_result.body();
        let assigned_params = collect_assigned_params(body);
        let mutated_params = mutation_scan.mutated(local_id);

        for (param_idx, param) in by_ref_params {
            let param_name_lower = param.name.as_str().fold_lower();

            if assigned_params.contains(&param_name_lower) {
                continue;
            }

            let param_range = ctx
                .item_tree()
                .method(local_id)
                .and_then(|item| item.params().get(param_idx))
                .map(|param_info| param_info.name_range);

            let Some(range) = param_range else {
                continue;
            };

            // The server method fills the argument for the caller (#271): `Знач` would silently
            // drop that data on the way back, so the finding names the std 487 p.12 remedy -
            // return the data from a function - and offers no quick fix.
            if mutated_params.contains(&param_name_lower) {
                diagnostics.push(Diagnostic {
                    code,
                    message: format!(
                        "Параметр {} метода {} возвращает данные на клиент: верните их результатом функции вместо изменения параметра",
                        param.name.as_str(),
                        method.name.as_str()
                    ),
                    severity: ctx.severity(code),
                    range,
                    tags: ctx.tags(code),
                    fixes: Vec::new(),
                });
                continue;
            }

            diagnostics.push(Diagnostic {
                code,
                message: format!(
                    "Установите модификатор \"Знач\" для параметра {} метода {}",
                    param.name.as_str(),
                    method.name.as_str()
                ),
                severity: ctx.severity(code),
                range,
                tags: ctx.tags(code),
                // Passing by value changes what the callee may do to the argument, so the
                // fix is opt-in and excluded from `source.fixAll`.
                fixes: vec![Fix::manual(
                    format!(
                        "Установить модификатор \"Знач\" для параметра {}",
                        param.name.as_str()
                    ),
                    vec![TextEdit {
                        range: TextRange::empty(range.start()),
                        new_text: "Знач ".to_string(),
                    }],
                )],
            });
        }
    }

    diagnostics
}

/// Methods of the mutable platform collections (Массив, Структура, Соответствие, СписокЗначений,
/// ТаблицаЗначений, ДеревоЗначений and their rows/columns collections) that change the
/// collection in place; lower-case, both spellings.
const MUTATING_METHODS: &[&str] = &[
    "добавить",
    "add",
    "вставить",
    "insert",
    "удалить",
    "delete",
    "очистить",
    "clear",
    "установить",
    "set",
    "загрузить",
    "load",
    "загрузитьзначения",
    "loadvalues",
    "загрузитьколонку",
    "loadcolumn",
    "заполнитьзначения",
    "fillvalues",
    "заполнитьпометки",
    "fillchecks",
    "свернуть",
    "groupby",
    "сортировать",
    "sort",
    "сортироватьпозначению",
    "sortbyvalue",
    "сортироватьпопредставлению",
    "sortbypresentation",
    "сдвинуть",
    "move",
];

/// Methods that return a new collection: changing their result leaves the source untouched.
const COPYING_METHODS: &[&str] = &[
    "скопировать",
    "copy",
    "скопироватьколонки",
    "copycolumns",
    "выгрузить",
    "unload",
    "выгрузитьзначения",
    "unloadvalues",
    "выгрузитьколонку",
    "unloadcolumn",
];

/// Names each method of the module changes in place. A name is changed when a field or element
/// reached from it is assigned (`П.X = ...`, `П[...] = ...`, `П.Строки.Найти(1).Сумма = ...`),
/// when a mutating collection method is called on something reached from it
/// (`П.Добавить(...)`, `П.Товары.Очистить()`), when it is passed to a method of the module that
/// changes the matching parameter, or when a local alias of it (`Х = П.Товары`,
/// `Для Каждого Х Из П`) is changed. Names the method reassigns are excluded: changing a new
/// value does not reach the caller's one.
struct MutationScan<'a> {
    module_bodies: &'a hir::ModuleBodies,
    /// Lower-case method name -> its key and lower-case parameter names.
    methods: FxHashMap<String, (hir::MethodKey, Vec<String>)>,
    done: FxHashMap<hir::MethodKey, FxHashSet<String>>,
    in_progress: FxHashSet<hir::MethodKey>,
}

impl<'a> MutationScan<'a> {
    fn new(module_bodies: &'a hir::ModuleBodies, symbol_tree: &hir::SymbolTree) -> Self {
        let methods = symbol_tree
            .methods()
            .map(|method| {
                let params = method.params.iter().map(|p| p.name.as_str().fold_lower()).collect();
                (method.name.as_str().fold_lower(), (method.id.local_id, params))
            })
            .collect();
        Self {
            module_bodies,
            methods,
            done: FxHashMap::default(),
            in_progress: FxHashSet::default(),
        }
    }

    fn mutated(&mut self, key: hir::MethodKey) -> FxHashSet<String> {
        if let Some(mutated) = self.done.get(&key) {
            return mutated.clone();
        }
        // A recursive call adds nothing the outer scan of the same method does not already see.
        if !self.in_progress.insert(key) {
            return FxHashSet::default();
        }
        let module_bodies = self.module_bodies;
        let mutated = match module_bodies.lower_result(key) {
            Some(lower_result) => {
                let body = lower_result.body();
                let assigned = collect_assigned_params(body);
                let mut mutated = self.scan_body(body);
                mutated.retain(|name| !assigned.contains(name));
                mutated
            }
            None => FxHashSet::default(),
        };
        self.in_progress.remove(&key);
        self.done.insert(key, mutated.clone());
        mutated
    }

    fn scan_body(&mut self, body: &hir::Body) -> FxHashSet<String> {
        let mut changed = FxHashSet::default();
        // (alias, name it was taken from): changing the alias changes that name's value.
        let mut aliases = Vec::new();

        for (_stmt_id, stmt) in body.stmts_iter() {
            match stmt {
                Stmt::Assign { target, value } => {
                    let target_id = hir::ExprId::from_idx(*target);
                    match body.expr(target_id) {
                        Expr::Path(name) => {
                            if let Some(source) =
                                access_chain_root(body, hir::ExprId::from_idx(*value))
                            {
                                aliases.push((name.as_str().fold_lower(), source));
                            }
                        }
                        Expr::Field { .. } | Expr::Index { .. } => {
                            changed.extend(access_chain_root(body, target_id));
                        }
                        _ => {}
                    }
                }
                Stmt::ForEach { var, collection, .. } => {
                    if let Some(source) =
                        access_chain_root(body, hir::ExprId::from_idx(*collection))
                    {
                        aliases.push((body.binding_idx(*var).name.as_str().fold_lower(), source));
                    }
                }
                _ => {}
            }
        }

        for (_expr_id, expr) in body.exprs_iter() {
            // `П.Добавить(...)` is lowered as a call of the field `Добавить` of `П`.
            let (receiver, method) = match expr {
                Expr::MethodCall { receiver, method, .. } => (*receiver, method),
                Expr::Call { callee, args } => match body.expr(hir::ExprId::from_idx(*callee)) {
                    Expr::Field { base, field } => (*base, field),
                    Expr::Path(name) => {
                        self.mark_passed_args(body, name.as_str(), args, &mut changed);
                        continue;
                    }
                    _ => continue,
                },
                _ => continue,
            };
            if MUTATING_METHODS.contains(&method.as_str().fold_lower().as_str()) {
                changed.extend(access_chain_root(body, hir::ExprId::from_idx(receiver)));
            }
        }

        loop {
            let before = changed.len();
            for (alias, source) in &aliases {
                if changed.contains(alias) {
                    changed.insert(source.clone());
                }
            }
            if changed.len() == before {
                return changed;
            }
        }
    }

    /// Marks the arguments of a call to a module method that changes the matching parameter.
    fn mark_passed_args(
        &mut self,
        body: &hir::Body,
        callee: &str,
        args: &[hir::ExprIdx],
        changed: &mut FxHashSet<String>,
    ) {
        let Some((key, params)) = self.methods.get(&callee.fold_lower()).cloned() else {
            return;
        };
        let callee_mutated = self.mutated(key);
        for (arg, param) in args.iter().zip(&params) {
            if callee_mutated.contains(param) {
                changed.extend(access_chain_root(body, hir::ExprId::from_idx(*arg)));
            }
        }
    }
}

/// Name at the root of a `П.X[...].Метод()...` chain of field, index and method-call accesses.
/// A copying method (`П.Скопировать()`) ends the chain: its result is not the root's value.
fn access_chain_root(body: &hir::Body, mut expr_id: hir::ExprId) -> Option<String> {
    loop {
        let base = match body.expr(expr_id) {
            Expr::Path(name) => return Some(name.as_str().fold_lower()),
            Expr::Field { base, .. } | Expr::Index { base, .. } => *base,
            Expr::MethodCall { receiver, method, .. } => {
                if COPYING_METHODS.contains(&method.as_str().fold_lower().as_str()) {
                    return None;
                }
                *receiver
            }
            Expr::Call { callee, .. } => match body.expr(hir::ExprId::from_idx(*callee)) {
                Expr::Field { base, field }
                    if !COPYING_METHODS.contains(&field.as_str().fold_lower().as_str()) =>
                {
                    *base
                }
                _ => return None,
            },
            _ => return None,
        };
        expr_id = hir::ExprId::from_idx(base);
    }
}

fn collect_assigned_params(body: &hir::Body) -> FxHashSet<String> {
    let mut assigned = FxHashSet::default();

    for (_stmt_id, stmt) in body.stmts_iter() {
        collect_assigned_from_stmt(stmt, body, &mut assigned);
    }

    assigned
}

fn collect_assigned_from_stmt(stmt: &Stmt, body: &hir::Body, assigned: &mut FxHashSet<String>) {
    match stmt {
        Stmt::Assign { target, .. } => {
            let target_id = hir::ExprId::from_idx(*target);
            if let Expr::Path(name) = body.expr(target_id) {
                assigned.insert(name.as_str().fold_lower());
            }
        }
        Stmt::If(if_stmt) => {
            for &stmt_idx in if_stmt.then_branch.iter() {
                let stmt_id = hir::StmtId::from_idx(stmt_idx);
                collect_assigned_from_stmt(body.stmt(stmt_id), body, assigned);
            }
            for (_, elsif_stmts) in if_stmt.elsif_branches.iter() {
                for &stmt_idx in elsif_stmts.iter() {
                    let stmt_id = hir::StmtId::from_idx(stmt_idx);
                    collect_assigned_from_stmt(body.stmt(stmt_id), body, assigned);
                }
            }
            if let Some(ref else_branch) = if_stmt.else_branch {
                for &stmt_idx in else_branch.iter() {
                    let stmt_id = hir::StmtId::from_idx(stmt_idx);
                    collect_assigned_from_stmt(body.stmt(stmt_id), body, assigned);
                }
            }
        }
        Stmt::While { body: stmts, .. }
        | Stmt::For { body: stmts, .. }
        | Stmt::ForEach { body: stmts, .. } => {
            for &stmt_idx in stmts.iter() {
                let stmt_id = hir::StmtId::from_idx(stmt_idx);
                collect_assigned_from_stmt(body.stmt(stmt_id), body, assigned);
            }
        }
        Stmt::Try { body: try_block, except, .. } => {
            for &stmt_idx in try_block.iter() {
                let stmt_id = hir::StmtId::from_idx(stmt_idx);
                collect_assigned_from_stmt(body.stmt(stmt_id), body, assigned);
            }
            for &stmt_idx in except.iter() {
                let stmt_id = hir::StmtId::from_idx(stmt_idx);
                collect_assigned_from_stmt(body.stmt(stmt_id), body, assigned);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{check_diagnostics_snapshot_for, check_hir_diagnostic};
    use expect_test::expect;

    #[test]
    fn fix_inserts_znach_before_param() {
        let code = "&НаКлиенте\nПроцедура Клиент()\n    Сервер(2);\nКонецПроцедуры\n\n&НаСервере\nПроцедура Сервер(Парам1)\nКонецПроцедуры\n";
        let diags: Vec<_> = check_hir_diagnostic(code)
            .into_iter()
            .filter(|d| d.code == DiagnosticCode::TransferringParametersBetweenClientAndServer)
            .collect();
        assert_eq!(diags.len(), 1, "expected exactly one diagnostic");
        assert_eq!(diags[0].fixes.len(), 1, "expected a quick fix: {:?}", diags[0].fixes);
        let edit = &diags[0].fixes[0].edits[0];
        let start: usize = edit.range.start().into();
        let end: usize = edit.range.end().into();
        let mut s = code.to_string();
        s.replace_range(start..end, &edit.new_text);
        assert!(s.contains("Процедура Сервер(Знач Парам1)"), "got: {s}");
    }
    #[test]
    fn test_by_ref_param_in_server_method_called_from_client() {
        let code = r#"&НаКлиенте
Процедура Клиент1()
    Сервер1(2);
КонецПроцедуры

&НаСервере
Процедура Сервер1(Парам1) // ошибка
    Метод(Парам1);
КонецПроцедуры

&НаКлиенте
Процедура Клиент2()
    Сервер2(2);
КонецПроцедуры

&НаСервере
Процедура Сервер2(Знач Парам1) // не ошибка
КонецПроцедуры

&НаКлиенте
Процедура Клиент3()
    Сервер3(2);
КонецПроцедуры

&НаСервере
Процедура Сервер3(Парам1) // не ошибка
    Парам1 = 10;
КонецПроцедуры

&НаКлиентеНаСервереБезКонтекста
Процедура КлиентСервер4(Парам1) // не ошибка
    Сервер4(2);
КонецПроцедуры

&НаСервере
Процедура Сервер4(Парам1) // не ошибка
    ЕщеМетод(Парам1);
КонецПроцедуры

&НаСервере
Процедура Метод(Парам1)
КонецПроцедуры

&НаСервере
Процедура ЕщеМетод(Парам1)
КонецПроцедуры
"#;

        check_diagnostics_snapshot_for(
            code,
            DiagnosticCode::TransferringParametersBetweenClientAndServer,
            expect![[r#"
                TransferringParametersBetweenClientAndServer @ 7:19..7:25
                  message: Установите модификатор "Знач" для параметра Парам1 метода Сервер1
                  severity: Warning"#]],
        );
    }

    #[test]
    fn test_no_diagnostic_for_by_value_param() {
        let code = r#"
&НаКлиенте
Процедура Клиент()
    Сервер(2);
КонецПроцедуры

&НаСервере
Процедура Сервер(Знач Парам)
    Результат = Парам + 1;
КонецПроцедуры
"#;

        check_diagnostics_snapshot_for(
            code,
            DiagnosticCode::TransferringParametersBetweenClientAndServer,
            expect![[r#""#]],
        );
    }

    #[test]
    fn test_no_diagnostic_when_param_assigned() {
        let code = r#"
&НаКлиенте
Процедура Клиент()
    Сервер(2);
КонецПроцедуры

&НаСервере
Процедура Сервер(Парам)
    Парам = 10;
КонецПроцедуры
"#;

        check_diagnostics_snapshot_for(
            code,
            DiagnosticCode::TransferringParametersBetweenClientAndServer,
            expect![[r#""#]],
        );
    }

    #[test]
    fn test_no_diagnostic_when_not_called_from_client() {
        let code = r#"
&НаКлиентеНаСервереБезКонтекста
Процедура КлиентСервер()
    Сервер(2);
КонецПроцедуры

&НаСервере
Процедура Сервер(Парам)
    Результат = Парам + 1;
КонецПроцедуры
"#;

        check_diagnostics_snapshot_for(
            code,
            DiagnosticCode::TransferringParametersBetweenClientAndServer,
            expect![[r#""#]],
        );
    }
}
