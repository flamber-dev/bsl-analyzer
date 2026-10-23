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

/// Methods that put their arguments into the collection they are called on.
const STORING_METHODS: &[&str] = &["добавить", "add", "вставить", "insert", "установить", "set"];

/// Platform function that writes the properties of its first argument.
const FILL_PROPERTY_VALUES: &[&str] = &["заполнитьзначениясвойств", "fillpropertyvalues"];

/// Parameters each method of the module changes in place, so that the caller sees the change.
///
/// Values are followed as sets of roots: a parameter `п` stands for the caller's value, `%п`
/// for the values stored inside it; `#г` and `%г` stand for the value a module variable `г`
/// holds when the method starts and the values stored inside it. A change is an assignment of a
/// field or element (`П.X = ...`, `П[...] = ...`), a mutating collection method
/// (`П.Добавить(...)`), `ЗаполнитьЗначенияСвойств(П, ...)` or a call of a module method that
/// changes the matching argument. Locals carry roots statement by statement (`Х = П.Товары`,
/// `Для Каждого Х Из П`, `Л.Товары = П`, `Х = ПолучитьТовары(П)`) until they are assigned
/// another value.
struct MutationScan {
    mutated: FxHashMap<hir::MethodKey, FxHashSet<String>>,
}

impl MutationScan {
    fn new(module_bodies: &hir::ModuleBodies, symbol_tree: &hir::SymbolTree) -> Self {
        let module_vars: FxHashSet<String> =
            symbol_tree.variables().map(|var| var.name.as_str().fold_lower()).collect();
        let mut methods = FxHashMap::default();
        let mut bodies = Vec::new();
        for method in symbol_tree.methods() {
            let key = method.id.local_id;
            let params: Vec<String> =
                method.params.iter().map(|p| p.name.as_str().fold_lower()).collect();
            if let Some(lower_result) = module_bodies.lower_result(key) {
                bodies.push((key, lower_result.body(), params.clone()));
            }
            methods.insert(method.name.as_str().fold_lower(), (key, params));
        }

        // Methods may call each other recursively, so the summaries grow together until none
        // changes; every root set only grows, so the iteration ends.
        let mut facts: FxHashMap<hir::MethodKey, MethodFacts> = FxHashMap::default();
        loop {
            let mut grew = false;
            for (key, body, params) in &bodies {
                let found = BodyScan::run(body, params, &module_vars, &methods, &facts);
                if facts.get(key) != Some(&found) {
                    facts.insert(*key, found);
                    grew = true;
                }
            }
            if !grew {
                break;
            }
        }

        let mutated = bodies
            .iter()
            .map(|(key, _, params)| {
                let changed = &facts[key].changed;
                let mutated = params
                    .iter()
                    .filter(|p| changed.contains(*p) || changed.contains(&contents_root(p)))
                    .cloned()
                    .collect();
                (*key, mutated)
            })
            .collect();
        Self { mutated }
    }

    fn mutated(&self, key: hir::MethodKey) -> Option<&FxHashSet<String>> {
        self.mutated.get(&key)
    }
}

fn contents_root(name: &str) -> String {
    format!("%{name}")
}

fn entry_root(module_var: &str) -> String {
    format!("#{module_var}")
}

type Roots = FxHashSet<String>;

/// What a call of a method does to the caller's values, in roots of that method.
#[derive(Default, PartialEq, Eq)]
struct MethodFacts {
    changed: Roots,
    returns: Value,
    /// Value each parameter and module variable carries when the method ends.
    exit: FxHashMap<String, Value>,
}

/// Roots of the value itself and of the values stored inside it.
#[derive(Clone, Default, PartialEq, Eq)]
struct Value {
    holds: Roots,
    contains: Roots,
}

impl Value {
    fn join(&mut self, other: Value) {
        self.holds.extend(other.holds);
        self.contains.extend(other.contains);
    }

    /// A field, element or method result of this value: one of the values stored inside it.
    fn part(self) -> Value {
        let mut all = self.holds;
        all.extend(self.contains);
        Value { holds: all.clone(), contains: all }
    }
}

/// Name -> value it may have at the current statement.
type State = FxHashMap<String, Value>;

fn join(states: impl IntoIterator<Item = State>) -> State {
    let mut joined = State::default();
    for state in states {
        for (name, value) in state {
            joined.entry(name).or_default().join(value);
        }
    }
    joined
}

#[derive(Default)]
struct LoopFrame {
    breaks: Option<State>,
    continues: Option<State>,
}

struct BodyScan<'a> {
    body: &'a hir::Body,
    module_vars: FxHashSet<String>,
    methods: &'a FxHashMap<String, (hir::MethodKey, Vec<String>)>,
    facts: &'a FxHashMap<hir::MethodKey, MethodFacts>,
    /// With `Перейти` any statement may follow any other: assignments only add roots.
    weak: bool,
    loops: Vec<LoopFrame>,
    /// States an exception handler of each enclosing `Попытка` may start from.
    tries: Vec<State>,
    exits: State,
    result: MethodFacts,
}

impl<'a> BodyScan<'a> {
    fn run(
        body: &'a hir::Body,
        params: &[String],
        module_vars: &FxHashSet<String>,
        methods: &'a FxHashMap<String, (hir::MethodKey, Vec<String>)>,
        facts: &'a FxHashMap<hir::MethodKey, MethodFacts>,
    ) -> MethodFacts {
        let locals: FxHashSet<String> = body
            .stmts_iter()
            .filter_map(|(_, stmt)| match stmt {
                Stmt::VarDecl { bindings } => Some(bindings),
                _ => None,
            })
            .flatten()
            .map(|&binding| body.binding_idx(binding).name.as_str().fold_lower())
            .chain(params.iter().cloned())
            .collect();
        let module_vars: FxHashSet<String> =
            module_vars.iter().filter(|var| !locals.contains(*var)).cloned().collect();

        let mut entry = State::default();
        for param in params {
            let value = Value {
                holds: Roots::from_iter([param.clone()]),
                contains: Roots::from_iter([contents_root(param)]),
            };
            entry.insert(param.clone(), value);
        }
        for var in &module_vars {
            let value = Value {
                holds: Roots::from_iter([entry_root(var)]),
                contains: Roots::from_iter([contents_root(var)]),
            };
            entry.insert(var.clone(), value);
        }

        let weak = body.stmts_iter().any(|(_, stmt)| matches!(stmt, Stmt::Goto(_)));
        let mut scan = Self {
            body,
            module_vars,
            methods,
            facts,
            weak,
            loops: Vec::new(),
            tries: Vec::new(),
            exits: State::default(),
            result: MethodFacts::default(),
        };
        let mut state = entry;
        if weak {
            scan.repeat(&mut state, |scan, state| scan.stmts(body.body_stmts_typed(), state));
        } else {
            scan.stmts(body.body_stmts_typed(), &mut state);
        }

        let exit = join([std::mem::take(&mut scan.exits), state]);
        scan.result.exit = params
            .iter()
            .chain(&scan.module_vars)
            .map(|name| (name.clone(), exit.get(name).cloned().unwrap_or_default()))
            .collect();
        scan.result
    }

    fn stmts(&mut self, stmts: &[hir::StmtIdx], state: &mut State) {
        for &stmt in stmts {
            self.stmt(self.body.stmt_idx(stmt), state);
        }
    }

    /// Runs `stmts` from a copy of `entry` and returns the state they leave.
    fn branch(&mut self, stmts: &[hir::StmtIdx], entry: &State) -> State {
        let mut state = entry.clone();
        self.stmts(stmts, &mut state);
        state
    }

    fn stmt(&mut self, stmt: &Stmt, state: &mut State) {
        for handler_entry in &mut self.tries {
            *handler_entry = join([std::mem::take(handler_entry), state.clone()]);
        }
        match stmt {
            Stmt::Expr(expr) | Stmt::Execute { expr } => self.expr(*expr, state),
            Stmt::Return { value } => {
                if let Some(value) = value {
                    self.expr(*value, state);
                    let returned = self.value(*value, state);
                    self.result.returns.join(returned);
                }
                self.exit(state);
            }
            Stmt::Raise { value } => {
                if let Some(value) = value {
                    self.expr(*value, state);
                }
                self.exit(state);
            }
            Stmt::AddHandler { event, handler } | Stmt::RemoveHandler { event, handler } => {
                self.expr(*event, state);
                self.expr(*handler, state);
            }
            Stmt::Assign { target, value } => {
                self.expr(*value, state);
                match self.body.expr_idx(*target) {
                    Expr::Path(name) => {
                        let value = self.value(*value, state);
                        self.rebind(state, name.as_str().fold_lower(), value);
                    }
                    Expr::Field { base, .. } | Expr::Index { base, .. } => {
                        let base = *base;
                        self.expr(*target, state);
                        let stored = self.value(*value, state);
                        self.change(base, state);
                        self.store(base, stored, state);
                    }
                    _ => self.expr(*target, state),
                }
            }
            Stmt::If(if_stmt) => {
                self.expr(if_stmt.condition, state);
                let mut exits = vec![self.branch(&if_stmt.then_branch, state)];
                for (condition, stmts) in if_stmt.elsif_branches.iter() {
                    self.expr(*condition, state);
                    exits.push(self.branch(stmts, state));
                }
                match &if_stmt.else_branch {
                    Some(stmts) => exits.push(self.branch(stmts, state)),
                    None => exits.push(state.clone()),
                }
                *state = join(exits);
            }
            Stmt::PreprocIf(preproc) => {
                let mut exits: Vec<_> =
                    preproc.branches().map(|branch| self.branch(branch.stmts, state)).collect();
                if preproc.else_branch.is_none() {
                    exits.push(state.clone());
                }
                *state = join(exits);
            }
            Stmt::While { condition, body } => {
                self.looped(state, |scan, state| {
                    scan.expr(*condition, state);
                    scan.stmts(body, state);
                });
            }
            Stmt::For { var, from, to, body } => {
                self.expr(*from, state);
                self.expr(*to, state);
                let var = self.body.binding_idx(*var).name.as_str().fold_lower();
                self.looped(state, |scan, state| {
                    scan.rebind(state, var.clone(), Value::default());
                    scan.stmts(body, state);
                });
            }
            Stmt::ForEach { var, collection, body } => {
                self.expr(*collection, state);
                let element = self.value(*collection, state).part();
                let var = self.body.binding_idx(*var).name.as_str().fold_lower();
                self.looped(state, |scan, state| {
                    scan.rebind(state, var.clone(), element.clone());
                    scan.stmts(body, state);
                });
            }
            Stmt::Try { body, except } => {
                self.tries.push(state.clone());
                let after_try = self.branch(body, state);
                let handler_entry = self.tries.pop().unwrap_or_default();
                let handler_entry = join([handler_entry, after_try.clone()]);
                let after_except = self.branch(except, &handler_entry);
                *state = join([after_try, after_except]);
            }
            Stmt::Break => {
                if let Some(frame) = self.loops.last_mut() {
                    let breaks = frame.breaks.take().unwrap_or_default();
                    frame.breaks = Some(join([breaks, state.clone()]));
                }
            }
            Stmt::Continue => {
                if let Some(frame) = self.loops.last_mut() {
                    let continues = frame.continues.take().unwrap_or_default();
                    frame.continues = Some(join([continues, state.clone()]));
                }
            }
            Stmt::VarDecl { .. } | Stmt::Goto(_) | Stmt::Label(_) => {}
        }
    }

    fn exit(&mut self, state: &State) {
        self.exits = join([std::mem::take(&mut self.exits), state.clone()]);
    }

    fn rebind(&self, state: &mut State, name: String, value: Value) {
        if self.weak {
            state.entry(name).or_default().join(value);
        } else {
            state.insert(name, value);
        }
    }

    /// Runs a loop body until the state it can leave stops growing; the loop may also run zero
    /// times, and `Прервать` leaves it from the middle.
    fn looped(&mut self, state: &mut State, mut body: impl FnMut(&mut Self, &mut State)) {
        loop {
            self.loops.push(LoopFrame::default());
            let mut end = state.clone();
            body(self, &mut end);
            let frame = self.loops.pop().unwrap_or_default();
            let next = join([state.clone(), end].into_iter().chain(frame.continues));
            if next == *state {
                *state = join([next].into_iter().chain(frame.breaks));
                return;
            }
            *state = next;
        }
    }

    /// Runs the whole body until the state stops growing.
    fn repeat(&mut self, state: &mut State, mut body: impl FnMut(&mut Self, &mut State)) {
        loop {
            let mut end = state.clone();
            body(self, &mut end);
            let next = join([state.clone(), end]);
            if next == *state {
                return;
            }
            *state = next;
        }
    }

    /// Walks the sub-expressions of `expr` in evaluation order and applies the calls among them.
    fn expr(&mut self, expr_idx: hir::ExprIdx, state: &mut State) {
        match self.body.expr_idx(expr_idx) {
            Expr::BinaryOp { lhs, rhs, .. } => {
                self.expr(*lhs, state);
                self.expr(*rhs, state);
            }
            Expr::UnaryOp { expr, .. } | Expr::Await { expr } => self.expr(*expr, state),
            Expr::Ternary { condition, then_expr, else_expr } => {
                self.expr(*condition, state);
                self.expr(*then_expr, state);
                self.expr(*else_expr, state);
            }
            Expr::Index { base, index } => {
                self.expr(*base, state);
                self.expr(*index, state);
            }
            Expr::Field { base, .. } => self.expr(*base, state),
            Expr::New { args, .. } | Expr::Array(args) => {
                for &arg in args.iter() {
                    self.expr(arg, state);
                }
            }
            Expr::MethodCall { receiver, method, args } => {
                self.expr(*receiver, state);
                for &arg in args.iter() {
                    self.expr(arg, state);
                }
                self.receiver_call(*receiver, method.as_str(), args, state);
            }
            Expr::Call { callee, args } => {
                for &arg in args.iter() {
                    self.expr(arg, state);
                }
                match self.body.expr_idx(*callee) {
                    // `П.Добавить(...)` is lowered as a call of the field `Добавить` of `П`.
                    Expr::Field { base, field } => {
                        self.expr(*base, state);
                        self.receiver_call(*base, field.as_str(), args, state);
                    }
                    Expr::Path(name) => self.bare_call(name.as_str(), args, state),
                    _ => self.expr(*callee, state),
                }
            }
            Expr::Missing | Expr::Literal(_) | Expr::Path(_) => {}
        }
    }

    fn receiver_call(
        &mut self,
        receiver: hir::ExprIdx,
        method: &str,
        args: &[hir::ExprIdx],
        state: &mut State,
    ) {
        let method = method.fold_lower();
        if !MUTATING_METHODS.contains(&method.as_str()) {
            return;
        }
        self.change(receiver, state);
        if STORING_METHODS.contains(&method.as_str()) {
            for &arg in args {
                let stored = self.value(arg, state).part();
                self.store(receiver, stored, state);
            }
        }
    }

    fn bare_call(&mut self, callee: &str, args: &[hir::ExprIdx], state: &mut State) {
        let callee = callee.fold_lower();
        let Some((key, params)) = self.methods.get(&callee) else {
            if !FILL_PROPERTY_VALUES.contains(&callee.as_str()) {
                return;
            }
            if let Some((&target, sources)) = args.split_first() {
                self.change(target, state);
                if let Some(&source) = sources.first() {
                    let stored = self.value(source, state).part();
                    self.store(target, stored, state);
                }
            }
            return;
        };
        let Some(facts) = self.facts.get(key) else {
            return;
        };
        let changed = self.translate(&facts.changed, params, args, state);
        self.result.changed.extend(changed);
        // `Знач` of the callee does not matter: inside one server call it copies the reference,
        // not the collection, so a change of the contents still reaches the argument.
        let mut updates = Vec::new();
        for (name, value) in &facts.exit {
            let translated = Value {
                holds: self.translate(&value.holds, params, args, state),
                contains: self.translate(&value.contains, params, args, state),
            };
            updates.push((name.clone(), translated));
        }
        for (name, value) in updates {
            if let Some(index) = params.iter().position(|param| *param == name) {
                // The callee may store values inside its argument; a new value it assigns to the
                // parameter itself never reaches the argument.
                if let Some(&arg) = args.get(index) {
                    let stored = Value { holds: value.contains.clone(), contains: value.contains };
                    self.store(arg, stored, state);
                }
            } else if self.module_vars.contains(&name) {
                self.rebind(state, name, value);
            }
        }
    }

    /// Roots of the callee mapped to roots of this method at the call.
    fn translate(
        &self,
        roots: &Roots,
        params: &[String],
        args: &[hir::ExprIdx],
        state: &State,
    ) -> Roots {
        let mut translated = Roots::default();
        for root in roots {
            let (contents, name) = match root.strip_prefix('%') {
                Some(name) => (true, name),
                None => (false, root.strip_prefix('#').unwrap_or(root)),
            };
            let value = match params.iter().position(|param| param == name) {
                Some(index) => match args.get(index) {
                    Some(&arg) => self.value(arg, state),
                    None => continue,
                },
                None if self.module_vars.contains(name) => {
                    state.get(name).cloned().unwrap_or_default()
                }
                // A module variable this method shadows keeps the value it had on entry.
                None => {
                    translated.insert(root.clone());
                    continue;
                }
            };
            translated.extend(if contents { value.contains } else { value.holds });
        }
        translated
    }

    /// Marks the value of `expr` as changed in place.
    fn change(&mut self, expr: hir::ExprIdx, state: &State) {
        let changed = self.value(expr, state).holds;
        self.result.changed.extend(changed);
    }

    /// Records that `stored` is put inside the value of `container`.
    fn store(&self, container: hir::ExprIdx, stored: Value, state: &mut State) {
        let Some(root) = access_chain_root(self.body, hir::ExprId::from_idx(container)) else {
            return;
        };
        let entry = state.entry(root).or_default();
        entry.contains.extend(stored.holds);
        entry.contains.extend(stored.contains);
    }

    /// Roots the value of `expr` may reach.
    fn value(&self, expr: hir::ExprIdx, state: &State) -> Value {
        match self.body.expr_idx(expr) {
            Expr::Path(name) => state.get(&name.as_str().fold_lower()).cloned().unwrap_or_default(),
            Expr::Field { base, .. } | Expr::Index { base, .. } => self.value(*base, state).part(),
            Expr::MethodCall { receiver, method, .. } => {
                self.method_result(*receiver, method.as_str(), state)
            }
            Expr::Call { callee, args } => match self.body.expr_idx(*callee) {
                Expr::Field { base, field } => self.method_result(*base, field.as_str(), state),
                Expr::Path(name) => {
                    let Some((key, params)) = self.methods.get(&name.as_str().fold_lower()) else {
                        return Value::default();
                    };
                    let Some(facts) = self.facts.get(key) else {
                        return Value::default();
                    };
                    Value {
                        holds: self.translate(&facts.returns.holds, params, args, state),
                        contains: self.translate(&facts.returns.contains, params, args, state),
                    }
                }
                _ => Value::default(),
            },
            Expr::Ternary { then_expr, else_expr, .. } => {
                let mut value = self.value(*then_expr, state);
                value.join(self.value(*else_expr, state));
                value
            }
            Expr::New { args, .. } | Expr::Array(args) => {
                let mut contains = Roots::default();
                for &arg in args.iter() {
                    let part = self.value(arg, state).part();
                    contains.extend(part.holds);
                }
                Value { holds: Roots::default(), contains }
            }
            Expr::Await { expr } => self.value(*expr, state),
            Expr::Missing | Expr::Literal(_) | Expr::BinaryOp { .. } | Expr::UnaryOp { .. } => {
                Value::default()
            }
        }
    }

    /// A method result is a part of the receiver unless the method copies it.
    fn method_result(&self, receiver: hir::ExprIdx, method: &str, state: &State) -> Value {
        if COPYING_METHODS.contains(&method.fold_lower().as_str()) {
            return Value::default();
        }
        self.value(receiver, state).part()
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
