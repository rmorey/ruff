use rustpython_ast::{Constant, Expr, ExprKind, Location, Operator};

use crate::ast::types::Range;
use crate::checkers::ast::Checker;
use crate::fix::Fix;
use crate::registry::Diagnostic;
use crate::source_code::Generator;
use crate::violations;

fn optional(expr: &Expr) -> Expr {
    Expr::new(
        Location::default(),
        Location::default(),
        ExprKind::BinOp {
            left: Box::new(expr.clone()),
            op: Operator::BitOr,
            right: Box::new(Expr::new(
                Location::default(),
                Location::default(),
                ExprKind::Constant {
                    value: Constant::None,
                    kind: None,
                },
            )),
        },
    )
}

fn union(elts: &[Expr]) -> Expr {
    if elts.len() == 1 {
        elts[0].clone()
    } else {
        Expr::new(
            Location::default(),
            Location::default(),
            ExprKind::BinOp {
                left: Box::new(union(&elts[..elts.len() - 1])),
                op: Operator::BitOr,
                right: Box::new(elts[elts.len() - 1].clone()),
            },
        )
    }
}

/// Returns `true` if any argument in the slice is a string.
fn any_arg_is_str(slice: &Expr) -> bool {
    match &slice.node {
        ExprKind::Constant {
            value: Constant::Str(_),
            ..
        } => true,
        ExprKind::Tuple { elts, .. } => elts.iter().any(any_arg_is_str),
        _ => false,
    }
}

enum TypingMember {
    Union,
    Optional,
}

/// UP007
pub fn use_pep604_annotation(checker: &mut Checker, expr: &Expr, value: &Expr, slice: &Expr) {
    // Avoid rewriting forward annotations.
    if any_arg_is_str(slice) {
        return;
    }

    let Some(typing_member) = checker.resolve_call_path(value).as_ref().and_then(|call_path| {
        if checker.match_typing_call_path(call_path, "Optional") {
            Some(TypingMember::Optional)
        } else if checker.match_typing_call_path(call_path, "Union") {
            Some(TypingMember::Union)
        } else {
            None
        }
    }) else {
        return;
    };

    match typing_member {
        TypingMember::Optional => {
            let mut diagnostic =
                Diagnostic::new(violations::UsePEP604Annotation, Range::from_located(expr));
            if checker.patch(diagnostic.kind.code()) {
                let mut generator: Generator = checker.stylist.into();
                generator.unparse_expr(&optional(slice), 0);
                diagnostic.amend(Fix::replacement(
                    generator.generate(),
                    expr.location,
                    expr.end_location.unwrap(),
                ));
            }
            checker.diagnostics.push(diagnostic);
        }
        TypingMember::Union => {
            let mut diagnostic =
                Diagnostic::new(violations::UsePEP604Annotation, Range::from_located(expr));
            if checker.patch(diagnostic.kind.code()) {
                match &slice.node {
                    ExprKind::Slice { .. } => {
                        // Invalid type annotation.
                    }
                    ExprKind::Tuple { elts, .. } => {
                        let mut generator: Generator = checker.stylist.into();
                        generator.unparse_expr(&union(elts), 0);
                        diagnostic.amend(Fix::replacement(
                            generator.generate(),
                            expr.location,
                            expr.end_location.unwrap(),
                        ));
                    }
                    _ => {
                        // Single argument.
                        let mut generator: Generator = checker.stylist.into();
                        generator.unparse_expr(slice, 0);
                        diagnostic.amend(Fix::replacement(
                            generator.generate(),
                            expr.location,
                            expr.end_location.unwrap(),
                        ));
                    }
                }
            }
            checker.diagnostics.push(diagnostic);
        }
    }
}
