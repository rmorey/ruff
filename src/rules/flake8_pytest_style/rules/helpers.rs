use num_traits::identities::Zero;
use rustpython_ast::{Constant, Expr, ExprKind, Keyword};

use crate::ast::helpers::collect_call_path;
use crate::checkers::ast::Checker;

const ITERABLE_INITIALIZERS: &[&str] = &["dict", "frozenset", "list", "tuple", "set"];

pub fn get_mark_decorators(decorators: &[Expr]) -> Vec<&Expr> {
    decorators
        .iter()
        .filter(|decorator| is_pytest_mark(decorator))
        .collect()
}

pub fn get_mark_name(decorator: &Expr) -> &str {
    collect_call_path(decorator).last().unwrap()
}

pub fn is_pytest_fail(call: &Expr, checker: &Checker) -> bool {
    checker.resolve_call_path(call).map_or(false, |call_path| {
        call_path.as_slice() == ["pytest", "fail"]
    })
}

pub fn is_pytest_fixture(decorator: &Expr, checker: &Checker) -> bool {
    checker
        .resolve_call_path(decorator)
        .map_or(false, |call_path| {
            call_path.as_slice() == ["pytest", "fixture"]
        })
}

pub fn is_pytest_mark(decorator: &Expr) -> bool {
    let segments = collect_call_path(decorator);
    if segments.len() > 2 {
        segments[0] == "pytest" && segments[1] == "mark"
    } else {
        false
    }
}

pub fn is_pytest_yield_fixture(decorator: &Expr, checker: &Checker) -> bool {
    checker
        .resolve_call_path(decorator)
        .map_or(false, |call_path| {
            call_path.as_slice() == ["pytest", "yield_fixture"]
        })
}

pub fn is_abstractmethod_decorator(decorator: &Expr, checker: &Checker) -> bool {
    checker
        .resolve_call_path(decorator)
        .map_or(false, |call_path| {
            call_path.as_slice() == ["abc", "abstractmethod"]
        })
}

/// Check if the expression is a constant that evaluates to false.
pub fn is_falsy_constant(expr: &Expr) -> bool {
    match &expr.node {
        ExprKind::Constant { value, .. } => match value {
            Constant::Bool(value) => !value,
            Constant::None => true,
            Constant::Str(string) => string.is_empty(),
            Constant::Bytes(bytes) => bytes.is_empty(),
            Constant::Int(int) => int.is_zero(),
            Constant::Float(float) => *float == 0.0,
            Constant::Complex { real, imag } => *real == 0.0 && *imag == 0.0,
            Constant::Ellipsis => true,
            Constant::Tuple(elts) => elts.is_empty(),
        },
        ExprKind::JoinedStr { values, .. } => values.is_empty(),
        ExprKind::Tuple { elts, .. } | ExprKind::List { elts, .. } => elts.is_empty(),
        ExprKind::Dict { keys, .. } => keys.is_empty(),
        ExprKind::Call {
            func,
            args,
            keywords,
        } => {
            if let ExprKind::Name { id, .. } = &func.node {
                if ITERABLE_INITIALIZERS.contains(&id.as_str()) {
                    if args.is_empty() && keywords.is_empty() {
                        return true;
                    } else if !keywords.is_empty() {
                        return false;
                    } else if let Some(arg) = args.get(0) {
                        return is_falsy_constant(arg);
                    }
                    return false;
                }
                false
            } else {
                false
            }
        }
        _ => false,
    }
}

pub fn is_pytest_parametrize(decorator: &Expr, checker: &Checker) -> bool {
    checker
        .resolve_call_path(decorator)
        .map_or(false, |call_path| {
            call_path.as_slice() == ["pytest", "mark", "parametrize"]
        })
}

pub fn keyword_is_literal(kw: &Keyword, literal: &str) -> bool {
    if let ExprKind::Constant {
        value: Constant::Str(string),
        ..
    } = &kw.node.value.node
    {
        string == literal
    } else {
        false
    }
}

pub fn is_empty_or_null_string(expr: &Expr) -> bool {
    match &expr.node {
        ExprKind::Constant {
            value: Constant::Str(string),
            ..
        } => string.is_empty(),
        ExprKind::Constant {
            value: Constant::None,
            ..
        } => true,
        ExprKind::JoinedStr { values } => values.iter().all(is_empty_or_null_string),
        _ => false,
    }
}
