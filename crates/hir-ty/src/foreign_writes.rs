//! The writes of one body that reaching definitions cannot read, described for the engine.
//!
//! "May this local have been written by something reaching definitions does not see" is a
//! reaching-definitions question: it is about which writes arrive at a point, and only the CFG
//! orders them. So this module answers the part that needs the resolver - which arguments a
//! callee may write back through - and hands the rest to `dataflow`, exactly as
//! [`crate::definite_init`] hands it [`dataflow::definite_init::InitEvents`].
//!
//! The direction arrives as a closure, because the bindings of a call are filled in by
//! inference as statements are inferred: a call not reached yet has none and reads as "not
//! proven by value", which keeps it a possible writer.

use bsl_platform::ParamPassing;
use cfg_types::IdConversion;
use dataflow::definite_init::InitPoint;
use dataflow::reaching_defs::{ForeignWrite, ForeignWriteEvents};
use hir_def::body::Body;
use hir_def::hir::{Expr, Stmt};
use hir_def::ExprId;
use smol_str::SmolStr;

use crate::definite_init::{call_shape, stmt_expr_roots, CallShape};

/// One pass over the body: every place that may write a local the body does not spell out.
///
/// Only an argument that is a bare variable path can be written back through - `Ф(Х.Поле)` and
/// `Ф(Х[1])` write into the object, not into the local - and only one not PROVEN to be passed
/// by value: an unresolved or unattested callee proves nothing, so it stays a writer. The two
/// are told apart, because a reader may look past an unattested direction and not past a
/// proven by-reference one.
pub(crate) fn collect(
    body: &Body,
    passing: &impl Fn(ExprId) -> Option<ParamPassing>,
) -> ForeignWriteEvents {
    let mut events = ForeignWriteEvents::default();

    for (stmt_id, stmt) in body.stmts_iter() {
        if matches!(stmt, Stmt::Execute { .. }) {
            events.push_every_name(stmt_id);
        }

        for (point, root) in stmt_expr_roots(body, stmt_id, stmt) {
            let CallShape { arguments, .. } = call_shape(body, root);
            // The set is unordered; the events of one place are a set too, so the only thing
            // the order could disturb is the order of equal definitions in the index.
            let mut written: Vec<(SmolStr, ForeignWrite)> = Vec::new();
            for node in arguments {
                let Expr::Path(name) = body.expr_idx(node) else { continue };
                let kind = match passing(ExprId::from_idx(node)) {
                    Some(ParamPassing::ByVal) => continue,
                    Some(ParamPassing::ByRef) => ForeignWrite::ByRefArgument,
                    // `ParamPassing` is non-exhaustive, and `None` means the corpus attests
                    // nothing: a passing we cannot name is not proof of a copy, so the argument
                    // stays a possible write - which is what `ArgPassing::Unknown` meant here.
                    _ => ForeignWrite::UnattestedArgument,
                };
                written.push((SmolStr::new(name.as_str()), kind));
            }
            written.sort_unstable_by(|left, right| left.0.cmp(&right.0));
            for (var_name, kind) in written {
                match point {
                    InitPoint::Stmt(site) => events.push_stmt(site, var_name, kind),
                    InitPoint::Header(header) => {
                        events.push_header(header, stmt_id, var_name, kind)
                    }
                }
            }
        }
    }

    events
}

/// Reaching definitions of one body that know about the writes above, for a caller that already
/// holds the CFG and the inference result - the cached queries of `ide-db`.
pub fn solve_with_cfg(
    db: &dyn crate::db::HirDatabase,
    cfg: std::sync::Arc<cfg::ControlFlowGraph>,
    body: &std::sync::Arc<Body>,
    bindings: &[crate::infer::CallArgBinding],
    seed_params: bool,
) -> Option<std::sync::Arc<dataflow::reaching_defs::ReachingDefsResult>> {
    let writes = collect(body, &|arg| crate::infer::argument_passing_of(db, bindings, arg));
    dataflow::reaching_defs::solve_executable_reaching_definitions_with_writes(
        cfg,
        body,
        seed_params,
        std::sync::Arc::new(writes),
    )
}

/// Reaching definitions of one body that know about the writes above, solved locally.
///
/// Inference cannot read the cached query for this: the collector needs the resolved passing
/// directions, so the query depends on the inference result that would be asking.
pub(crate) fn solve_locally(
    body: &std::sync::Arc<Body>,
    seed_params: bool,
    passing: &impl Fn(ExprId) -> Option<ParamPassing>,
) -> Option<std::sync::Arc<dataflow::reaching_defs::ReachingDefsResult>> {
    let cfg = cfg::CfgBuilder::new().build_graph_from_hir(body.body_stmts_typed(), body);
    let writes = collect(body, passing);
    dataflow::reaching_defs::solve_executable_reaching_definitions_with_writes(
        std::sync::Arc::new(cfg),
        body,
        seed_params,
        std::sync::Arc::new(writes),
    )
}

/// Whether a write the analysis cannot read reaches the read of `key` at `stmt`.
///
/// A read inside an `Если` / `Пока` condition or a `Для` bound happens at the header, which is
/// no statement of any block: its definitions are asked by the expression the header evaluates
/// first. Definitions that cannot be obtained answer `true`: the absence of a foreign write is
/// then unproven, and a lost finding is cheaper than an invented one.
pub(crate) fn foreign_write_reaches(
    defs: &dataflow::reaching_defs::ReachingDefsResult,
    body: &Body,
    key: &str,
    stmt: hir_def::StmtId,
) -> bool {
    let header = dataflow::reaching_defs::header_read_expr(body, stmt);
    let reaching = match header {
        Some(header) => defs.defs_for_var_at_header(key, header),
        None => defs.defs_for_var_at_stmt(key, stmt),
    };
    match reaching {
        Some(reaching) => reaching.iter().any(|def| {
            matches!(def.def_site, dataflow::reaching_defs::DefSite::ForeignWrite { .. })
        }),
        None => true,
    }
}
