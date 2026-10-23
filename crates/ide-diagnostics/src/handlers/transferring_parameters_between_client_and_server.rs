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
    let mut mutation_scan: Option<MutationScan> = None;
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
        let mutated_params = mutation_scan
            .get_or_insert_with(|| MutationScan::new(&module_bodies, &symbol_tree))
            .mutated(local_id);

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
            if mutated_params.is_some_and(|mutated| mutated.contains(&param_name_lower)) {
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

/// Parameters each method of the module changes in place, so that the caller sees the change.
///
/// A parameter is changed when a field or element reached from it is assigned (`П.X = ...`,
/// `П[...] = ...`, `П.Строки.Найти(1).Сумма = ...`), when a mutating collection method is called
/// on something reached from it (`П.Добавить(...)`, `П.Товары.Очистить()`), or when it is passed
/// to a method of the module that changes the matching parameter. A local that holds a part of
/// the parameter (`Х = П.Товары`, `Для Каждого Х Из П`) changes the parameter too, until it or
/// the parameter is assigned another value.
struct MutationScan {
    mutated: FxHashMap<hir::MethodKey, FxHashSet<String>>,
}

impl MutationScan {
    fn new(module_bodies: &hir::ModuleBodies, symbol_tree: &hir::SymbolTree) -> Self {
        let methods: FxHashMap<String, (hir::MethodKey, Vec<String>)> = symbol_tree
            .methods()
            .map(|method| {
                let params = method.params.iter().map(|p| p.name.as_str().fold_lower()).collect();
                (method.name.as_str().fold_lower(), (method.id.local_id, params))
            })
            .collect();
        let bodies: Vec<_> = symbol_tree
            .methods()
            .filter_map(|method| {
                let key = method.id.local_id;
                module_bodies.lower_result(key).map(|lower_result| (key, lower_result.body()))
            })
            .collect();

        // Methods may call each other recursively, so the sets grow together until none changes;
        // a callee's set only ever grows, so every rescan finds a superset of the previous one.
        let mut mutated: FxHashMap<hir::MethodKey, FxHashSet<String>> = FxHashMap::default();
        loop {
            let mut grew = false;
            for &(key, body) in &bodies {
                let found = BodyScan::run(body, &methods, &mutated);
                let known = mutated.entry(key).or_default();
                if found.len() != known.len() {
                    *known = found;
                    grew = true;
                }
            }
            if !grew {
                return Self { mutated };
            }
        }
    }

    fn mutated(&self, key: hir::MethodKey) -> Option<&FxHashSet<String>> {
        self.mutated.get(&key)
    }
}

/// Local name -> parameters whose caller value it may hold at the current statement.
type Aliases = FxHashMap<String, FxHashSet<String>>;

struct BodyScan<'a> {
    body: &'a hir::Body,
    methods: &'a FxHashMap<String, (hir::MethodKey, Vec<String>)>,
    callee_mutated: &'a FxHashMap<hir::MethodKey, FxHashSet<String>>,
    changed: FxHashSet<String>,
}

impl<'a> BodyScan<'a> {
    fn run(
        body: &'a hir::Body,
        methods: &'a FxHashMap<String, (hir::MethodKey, Vec<String>)>,
        callee_mutated: &'a FxHashMap<hir::MethodKey, FxHashSet<String>>,
    ) -> FxHashSet<String> {
        let mut scan = Self { body, methods, callee_mutated, changed: FxHashSet::default() };
        let mut aliases: Aliases = body
            .params()
            .map(|param| {
                let name = body.binding(param).name.as_str().fold_lower();
                (name.clone(), FxHashSet::from_iter([name]))
            })
            .collect();
        scan.stmts(body.body_stmts_typed(), &mut aliases);
        scan.changed
    }

    fn stmts(&mut self, stmts: &[hir::StmtIdx], aliases: &mut Aliases) {
        for &stmt in stmts {
            self.stmt(self.body.stmt_idx(stmt), aliases);
        }
    }

    /// Runs `stmts` from a copy of `entry` and returns the state they leave.
    fn branch(&mut self, stmts: &[hir::StmtIdx], entry: &Aliases) -> Aliases {
        let mut aliases = entry.clone();
        self.stmts(stmts, &mut aliases);
        aliases
    }

    fn stmt(&mut self, stmt: &Stmt, aliases: &mut Aliases) {
        match stmt {
            Stmt::Expr(expr)
            | Stmt::Execute { expr }
            | Stmt::Return { value: Some(expr) }
            | Stmt::Raise { value: Some(expr) } => self.expr(*expr, aliases),
            Stmt::AddHandler { event, handler } | Stmt::RemoveHandler { event, handler } => {
                self.expr(*event, aliases);
                self.expr(*handler, aliases);
            }
            Stmt::Assign { target, value } => {
                self.expr(*value, aliases);
                match self.body.expr_idx(*target) {
                    Expr::Path(name) => {
                        let holds = self.roots(*value, aliases);
                        rebind(aliases, name.as_str().fold_lower(), holds);
                    }
                    Expr::Field { .. } | Expr::Index { .. } => {
                        self.expr(*target, aliases);
                        let changed = self.roots(*target, aliases);
                        self.changed.extend(changed);
                    }
                    _ => self.expr(*target, aliases),
                }
            }
            Stmt::If(if_stmt) => {
                self.expr(if_stmt.condition, aliases);
                let mut exits = vec![self.branch(&if_stmt.then_branch, aliases)];
                for (condition, stmts) in if_stmt.elsif_branches.iter() {
                    self.expr(*condition, aliases);
                    exits.push(self.branch(stmts, aliases));
                }
                match &if_stmt.else_branch {
                    Some(stmts) => exits.push(self.branch(stmts, aliases)),
                    None => exits.push(aliases.clone()),
                }
                *aliases = join(exits);
            }
            Stmt::PreprocIf(preproc) => {
                let mut exits: Vec<_> =
                    preproc.branches().map(|branch| self.branch(branch.stmts, aliases)).collect();
                if preproc.else_branch.is_none() {
                    exits.push(aliases.clone());
                }
                *aliases = join(exits);
            }
            Stmt::While { condition, body } => {
                self.repeat(aliases, |scan, state| {
                    scan.expr(*condition, state);
                    scan.stmts(body, state);
                });
            }
            Stmt::For { var, from, to, body } => {
                self.expr(*from, aliases);
                self.expr(*to, aliases);
                let var = self.body.binding_idx(*var).name.as_str().fold_lower();
                self.repeat(aliases, |scan, state| {
                    rebind(state, var.clone(), FxHashSet::default());
                    scan.stmts(body, state);
                });
            }
            Stmt::ForEach { var, collection, body } => {
                self.expr(*collection, aliases);
                let holds = self.roots(*collection, aliases);
                let var = self.body.binding_idx(*var).name.as_str().fold_lower();
                self.repeat(aliases, |scan, state| {
                    rebind(state, var.clone(), holds.clone());
                    scan.stmts(body, state);
                });
            }
            Stmt::Try { body, except } => {
                let after_try = self.branch(body, aliases);
                // The handler may start after any statement of the block.
                let handler_entry = join(vec![aliases.clone(), after_try.clone()]);
                let after_except = self.branch(except, &handler_entry);
                *aliases = join(vec![after_try, after_except]);
            }
            _ => {}
        }
    }

    /// Runs a loop body until the aliases it can leave stop growing; the loop may also run zero
    /// times, so the entry state stays part of the result.
    fn repeat(&mut self, aliases: &mut Aliases, mut body: impl FnMut(&mut Self, &mut Aliases)) {
        loop {
            let mut state = aliases.clone();
            body(self, &mut state);
            let joined = join(vec![aliases.clone(), state]);
            if joined == *aliases {
                return;
            }
            *aliases = joined;
        }
    }

    fn expr(&mut self, expr_idx: hir::ExprIdx, aliases: &Aliases) {
        match self.body.expr_idx(expr_idx) {
            Expr::BinaryOp { lhs, rhs, .. } => {
                self.expr(*lhs, aliases);
                self.expr(*rhs, aliases);
            }
            Expr::UnaryOp { expr, .. } | Expr::Await { expr } => self.expr(*expr, aliases),
            Expr::Ternary { condition, then_expr, else_expr } => {
                self.expr(*condition, aliases);
                self.expr(*then_expr, aliases);
                self.expr(*else_expr, aliases);
            }
            Expr::Index { base, index } => {
                self.expr(*base, aliases);
                self.expr(*index, aliases);
            }
            Expr::Field { base, .. } => self.expr(*base, aliases),
            Expr::New { args, .. } | Expr::Array(args) => {
                for &arg in args.iter() {
                    self.expr(arg, aliases);
                }
            }
            Expr::MethodCall { receiver, method, args } => {
                self.expr(*receiver, aliases);
                for &arg in args.iter() {
                    self.expr(arg, aliases);
                }
                self.mutating_call(*receiver, method.as_str(), aliases);
            }
            Expr::Call { callee, args } => {
                for &arg in args.iter() {
                    self.expr(arg, aliases);
                }
                match self.body.expr_idx(*callee) {
                    // `П.Добавить(...)` is lowered as a call of the field `Добавить` of `П`.
                    Expr::Field { base, field } => {
                        self.expr(*base, aliases);
                        self.mutating_call(*base, field.as_str(), aliases);
                    }
                    Expr::Path(name) => self.module_method_call(name.as_str(), args, aliases),
                    _ => self.expr(*callee, aliases),
                }
            }
            Expr::Missing | Expr::Literal(_) | Expr::Path(_) => {}
        }
    }

    fn mutating_call(&mut self, receiver: hir::ExprIdx, method: &str, aliases: &Aliases) {
        if MUTATING_METHODS.contains(&method.fold_lower().as_str()) {
            let changed = self.roots(receiver, aliases);
            self.changed.extend(changed);
        }
    }

    /// A call of a module method changes every argument passed to a parameter it changes.
    /// `Знач` of the callee does not matter here: inside one server call it copies the
    /// reference, not the collection, so a change of the contents still reaches the argument.
    fn module_method_call(&mut self, callee: &str, args: &[hir::ExprIdx], aliases: &Aliases) {
        let Some((key, params)) = self.methods.get(&callee.fold_lower()) else {
            return;
        };
        let Some(callee_mutated) = self.callee_mutated.get(key) else {
            return;
        };
        for (&arg, param) in args.iter().zip(params) {
            if callee_mutated.contains(param) {
                let changed = self.roots(arg, aliases);
                self.changed.extend(changed);
            }
        }
    }

    /// Parameters whose caller value `expr` may reach.
    fn roots(&self, expr: hir::ExprIdx, aliases: &Aliases) -> FxHashSet<String> {
        access_chain_root(self.body, hir::ExprId::from_idx(expr))
            .and_then(|name| aliases.get(&name).cloned())
            .unwrap_or_default()
    }
}

fn rebind(aliases: &mut Aliases, name: String, holds: FxHashSet<String>) {
    if holds.is_empty() {
        aliases.remove(&name);
    } else {
        aliases.insert(name, holds);
    }
}

/// May-alias union of the states several control-flow paths leave.
fn join(states: Vec<Aliases>) -> Aliases {
    let mut joined = Aliases::default();
    for state in states {
        for (name, holds) in state {
            joined.entry(name).or_default().extend(holds);
        }
    }
    joined
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
