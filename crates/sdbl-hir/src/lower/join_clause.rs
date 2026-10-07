use crate::diagnostics::SdblDiagnostic;
use crate::hir::JoinHir;
use crate::hir::TableRef;
use syntax::ast::AstNode;

use super::context::LoweringContext;

impl LoweringContext<'_> {
    pub(super) fn lower_joins(&mut self, query: &syntax::ast::SdblQuery) -> Vec<JoinHir> {
        let Some(from_clause) = query.from_clause() else {
            return Vec::new();
        };

        let Some(first_ds) = from_clause.data_sources().next() else {
            return Vec::new();
        };

        let mut all_joins = Vec::new();
        for join in first_ds.join_clauses() {
            self.lower_join_clause_recursive(&join, &mut all_joins);
        }
        all_joins
    }

    fn lower_join_clause_recursive(
        &mut self,
        join: &syntax::ast::SdblJoinClause,
        out: &mut Vec<JoinHir>,
    ) {
        let join_hir = self.lower_join_clause(join);

        if let Some(ds) = join.data_source() {
            for nested_join in ds.join_clauses() {
                self.lower_join_clause_recursive(&nested_join, out);
            }
        }

        out.push(join_hir);
    }

    fn lower_join_clause(&mut self, join: &syntax::ast::SdblJoinClause) -> JoinHir {
        self.record_keyword_by_text(
            join.syntax(),
            "JOIN",
            "СОЕДИНЕНИЕ",
            crate::source_map::TokenCategory::JoinKeyword,
        );

        let ast_join_type = join.join_type();
        let join_type = match ast_join_type {
            syntax::ast::JoinType::Left => {
                self.record_keyword_by_text(
                    join.syntax(),
                    "LEFT",
                    "ЛЕВОЕ",
                    crate::source_map::TokenCategory::JoinKeyword,
                );
                crate::hir::JoinType::Left
            }
            syntax::ast::JoinType::Right => {
                self.record_keyword_by_text(
                    join.syntax(),
                    "RIGHT",
                    "ПРАВОЕ",
                    crate::source_map::TokenCategory::JoinKeyword,
                );
                crate::hir::JoinType::Right
            }
            syntax::ast::JoinType::Full => {
                self.record_keyword_by_text(
                    join.syntax(),
                    "FULL",
                    "ПОЛНОЕ",
                    crate::source_map::TokenCategory::JoinKeyword,
                );
                crate::hir::JoinType::Full
            }
            syntax::ast::JoinType::Inner => {
                self.record_keyword_by_text(
                    join.syntax(),
                    "INNER",
                    "ВНУТРЕННЕЕ",
                    crate::source_map::TokenCategory::JoinKeyword,
                );
                crate::hir::JoinType::Inner
            }
        };

        if matches!(join_type, crate::hir::JoinType::Full) {
            self.diagnostics
                .push(SdblDiagnostic::FullOuterJoin { range: join.syntax().text_range() });
        }

        let table = if let Some(ds) = join.data_source() {
            if let Some(subquery) = ds.subquery() {
                self.diagnostics.push(SdblDiagnostic::JoinWithSubQuery {
                    range: subquery.syntax().text_range(),
                });
            }

            self.lower_data_source(&ds)
        } else {
            TableRef::missing(join.syntax().text_range())
        };

        if table.is_virtual_table {
            self.report_virtual_table_join(&table.parts, table.range);
        }

        if let Some(alias) = self.scope.add_table(table.clone()) {
            self.diagnostics.push(SdblDiagnostic::DuplicateAlias { alias, range: table.range });
        }

        let condition_node = join.syntax().children().find(|n| {
            matches!(
                n.kind(),
                syntax::SyntaxKind::SDBL_LOGICAL_OR_EXPR
                    | syntax::SyntaxKind::SDBL_LOGICAL_AND_EXPR
                    | syntax::SyntaxKind::SDBL_COMPARISON_EXPR
                    | syntax::SyntaxKind::SDBL_IS_NULL_EXPR
                    | syntax::SyntaxKind::SDBL_IN_EXPR
                    | syntax::SyntaxKind::SDBL_BETWEEN_EXPR
                    | syntax::SyntaxKind::SDBL_LIKE_EXPR
            )
        });

        if let Some(condition) = &condition_node {
            self.report_or_in_join_condition(condition);
        }

        let condition = condition_node.map(|expr| self.lower_expr(&expr));

        JoinHir { join_type, table, condition, range: join.syntax().text_range() }
    }

    /// Reports a disjunction in a join condition unless it can be rewritten as `В (...)`.
    ///
    /// The join can use an index only for conditions joined by `И`; a disjunction over
    /// different subjects defeats it. A disjunction over one subject is the exception the
    /// query standard allows, because it is equivalent to a membership test. Nested queries
    /// inside the condition are part of the condition, so their disjunctions are judged too.
    fn report_or_in_join_condition(&mut self, condition: &syntax::SyntaxNode) {
        for node in condition.descendants() {
            if node.kind() != syntax::SyntaxKind::SDBL_LOGICAL_OR_EXPR {
                continue;
            }
            let or_tokens: Vec<_> = node
                .children_with_tokens()
                .filter_map(|element| element.into_token())
                .filter(|token| token.kind() == syntax::SyntaxKind::KW_OR)
                .collect();
            if or_tokens.is_empty() || disjunction_has_single_subject(&node) {
                continue;
            }
            for token in or_tokens {
                self.diagnostics
                    .push(SdblDiagnostic::LogicalOrInJoin { range: token.text_range() });
            }
        }
    }
}

/// A disjunction has a single subject when every operand constrains the same column
/// spelling and none of them computes the subject through a function, a pattern, a range
/// or a choice: only then can the platform turn it into one index lookup by a list.
fn disjunction_has_single_subject(or_node: &syntax::SyntaxNode) -> bool {
    let mut subject: Option<String> = None;
    for node in or_node.descendants() {
        match node.kind() {
            syntax::SyntaxKind::SDBL_FUNCTION_CALL
            | syntax::SyntaxKind::SDBL_LIKE_EXPR
            | syntax::SyntaxKind::SDBL_BETWEEN_EXPR
            | syntax::SyntaxKind::SDBL_CASE_EXPR => return false,
            syntax::SyntaxKind::SDBL_COLUMN_REF => {
                let spelling = node.text().to_string();
                match &subject {
                    Some(known) if *known != spelling => return false,
                    Some(_) => {}
                    None => subject = Some(spelling),
                }
            }
            _ => {}
        }
    }
    true
}
