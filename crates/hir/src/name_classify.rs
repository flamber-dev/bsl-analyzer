use syntax::ast_utils::{field_tail_name_token, new_expr_type_name_token};
use syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

/// What a token names syntactically. `is_call` marks a callee: of a bare call `Имя(...)` on
/// `FreeName`, of a member call `Объект.Имя(...)` on `FieldName`.
#[derive(Debug, Clone)]
pub enum NameClass {
    FreeName { token: SyntaxToken, is_call: bool },

    FieldName { receiver: SyntaxNode, token: SyntaxToken, is_call: bool },

    TypeRef { token: SyntaxToken },

    Literal { token: SyntaxToken },

    Keyword { token: SyntaxToken },

    Other,
}

pub fn classify_token(token: &SyntaxToken) -> NameClass {
    if let Some((field_expr, name_token)) = field_expr_for_tail(token) {
        if name_token == *token {
            if let Some(receiver) = field_expr.children().next() {
                let is_call = is_call_callee(&field_expr);
                return NameClass::FieldName { receiver, token: token.clone(), is_call };
            }
        }
    }

    if let Some((new_expr, type_token)) = new_expr_for_type_name(token) {
        if type_token == *token {
            let _ = new_expr;
            return NameClass::TypeRef { token: token.clone() };
        }
    }

    if token.kind().is_literal() {
        return NameClass::Literal { token: token.clone() };
    }

    if token.kind().is_keyword() {
        return NameClass::Keyword { token: token.clone() };
    }

    if token.kind() == SyntaxKind::IDENT {
        return NameClass::FreeName { token: token.clone(), is_call: is_bare_call_callee(token) };
    }

    NameClass::Other
}

fn field_expr_for_tail(token: &SyntaxToken) -> Option<(SyntaxNode, SyntaxToken)> {
    let parent = token.parent()?;
    if parent.kind() != SyntaxKind::FIELD_EXPR {
        return None;
    }
    let tail = field_tail_name_token(&parent)?;
    Some((parent, tail))
}

fn is_call_callee(field_expr: &SyntaxNode) -> bool {
    let Some(parent) = field_expr.parent() else { return false };
    if parent.kind() != SyntaxKind::CALL_EXPR {
        return false;
    }
    parent.children().next().map(|n| n == *field_expr).unwrap_or(false)
}

/// The callee of a bare call is the `IDENT` node that opens its `CALL_EXPR`; a procedure
/// statement `Имя();` wraps that same `CALL_EXPR` in a `CALL_STMT`.
pub(crate) fn is_bare_call_callee(token: &SyntaxToken) -> bool {
    let Some(ident) = token.parent().filter(|node| node.kind() == SyntaxKind::IDENT) else {
        return false;
    };
    ident.parent().is_some_and(|call| {
        call.kind() == SyntaxKind::CALL_EXPR && call.children().next().as_ref() == Some(&ident)
    })
}

/// The name a `Процедура` / `Функция` header declares.
pub(crate) fn is_method_declaration_name(token: &SyntaxToken) -> bool {
    token.parent().is_some_and(|node| {
        matches!(node.kind(), SyntaxKind::PROCEDURE_DEF | SyntaxKind::FUNCTION_DEF)
    })
}

fn new_expr_for_type_name(token: &SyntaxToken) -> Option<(SyntaxNode, SyntaxToken)> {
    let parent = token.parent()?;
    if parent.kind() != SyntaxKind::NEW_EXPR {
        return None;
    }
    let type_token = new_expr_type_name_token(&parent)?;
    Some((parent, type_token))
}
