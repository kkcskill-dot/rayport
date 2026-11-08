//! Conservative template subset, not an Oracle execution sandbox or cost estimator.
//! Unknown syntax, literal values and side-effect-capable constructs fail closed.
use crate::{DomainError, SqlInspection};
use sha2::{Digest, Sha256};
use sqlparser::{
    ast::{
        BinaryOperator, Expr, FunctionArg, FunctionArgExpr, FunctionArguments, GroupByExpr, Ident,
        JoinConstraint, JoinOperator, ObjectName, Query, Select, SelectFlavor, SelectItem, SetExpr,
        Statement, TableFactor, UnaryOperator, Value, ValueWithSpan, Visit, VisitMut, Visitor,
        VisitorMut,
    },
    dialect::GenericDialect,
    parser::Parser,
};
use std::{collections::BTreeSet, ops::ControlFlow};

fn rejected() -> DomainError {
    DomainError::new(
        "unsafe_sql",
        "바인드 기반의 제한된 SELECT 템플릿만 허용합니다. 미지원 구문은 별도 검토가 필요합니다.",
    )
}

pub fn inspect_sql(sql: &str) -> Result<SqlInspection, DomainError> {
    // Reject comments/hints instead of attempting to sanitize executable input.
    if sql.trim().is_empty()
        || sql.len() > 32_768
        || !sql.is_ascii()
        || sql.contains("--")
        || sql.contains("/*")
        || sql.contains("*/")
        || sql.contains('#')
        || sql
            .bytes()
            .any(|b| b.is_ascii_control() && !b"\t\r\n".contains(&b))
    {
        return Err(rejected());
    }
    let dialect = GenericDialect {};
    let mut statements = Parser::new(&dialect)
        .with_recursion_limit(48)
        .try_with_sql(sql)
        .map_err(|_| rejected())?
        .parse_statements()
        .map_err(|_| rejected())?;
    if statements.len() != 1 || !matches!(statements[0], Statement::Query(_)) {
        return Err(rejected());
    }
    let mut guard = TemplateGuard::default();
    if let ControlFlow::Break(()) = statements[0].visit(&mut guard) {
        return Err(rejected());
    }
    if guard.tables.is_empty() || guard.tables.len() > 4 || guard.parameters.is_empty() {
        return Err(rejected());
    }
    let _ = VisitMut::visit(&mut statements[0], &mut CanonicalNames);
    let normalized_sql = statements[0].to_string();
    let fingerprint = format!("{:x}", Sha256::digest(normalized_sql.as_bytes()));
    Ok(SqlInspection {
        normalized_sql,
        fingerprint,
        tables: guard.tables.into_iter().collect(),
        parameters: guard.parameters.into_iter().collect(),
    })
}

/// A bind appearing only in SELECT or a tautology is not a limiting predicate.
/// This establishes necessary syntax; approved source objects and row policies remain
/// the execution adapter/DBA's responsibility.
pub fn validate_apic_bind_guards(sql: &str) -> Result<(), DomainError> {
    let inspected = inspect_sql(sql)?;
    let statements =
        Parser::parse_sql(&GenericDialect {}, &inspected.normalized_sql).map_err(|_| rejected())?;
    let Statement::Query(query) = &statements[0] else {
        return Err(rejected());
    };
    let SetExpr::Select(select) = query.body.as_ref() else {
        return Err(rejected());
    };
    let selection = select.selection.as_ref().ok_or_else(rejected)?;
    for (name, operator) in [
        ("start_date", BinaryOperator::GtEq),
        ("end_date_exclusive", BinaryOperator::Lt),
        ("store_id", BinaryOperator::Eq),
    ] {
        if !has_bound_conjunct(selection, name, &operator) {
            return Err(DomainError::new(
                "missing_bind_guard",
                "기간과 점포 바인드는 WHERE의 필수 AND 조건에 직접 적용되어야 합니다.",
            ));
        }
    }
    Ok(())
}

fn has_bound_conjunct(expression: &Expr, name: &str, required: &BinaryOperator) -> bool {
    match expression {
        Expr::Nested(expression) => has_bound_conjunct(expression, name, required),
        Expr::BinaryOp {
            left,
            op: BinaryOperator::And,
            right,
        } => has_bound_conjunct(left, name, required) || has_bound_conjunct(right, name, required),
        Expr::BinaryOp { left, op, right } if op == required => {
            matches!(
                left.as_ref(),
                Expr::Identifier(_) | Expr::CompoundIdentifier(_)
            ) && matches!(right.as_ref(), Expr::Value(value) if matches!(&value.value, Value::Placeholder(value) if value == &format!(":{name}")))
        }
        _ => false,
    }
}

#[derive(Default)]
struct TemplateGuard {
    queries: usize,
    expression_depth: usize,
    expression_count: usize,
    tables: BTreeSet<String>,
    parameters: BTreeSet<String>,
}

impl Visitor for TemplateGuard {
    type Break = ();

    fn pre_visit_query(&mut self, query: &Query) -> ControlFlow<()> {
        self.queries += 1;
        if self.queries != 1
            || query.with.is_some()
            || !matches!(query.body.as_ref(), SetExpr::Select(_))
            || query.fetch.is_some()
            || query.limit_clause.is_some()
            || !query.locks.is_empty()
            || query.for_clause.is_some()
            || query.settings.is_some()
            || query.format_clause.is_some()
            || !query.pipe_operators.is_empty()
        {
            return ControlFlow::Break(());
        }
        ControlFlow::Continue(())
    }

    fn pre_visit_select(&mut self, select: &Select) -> ControlFlow<()> {
        if select.into.is_some()
            || select.from.len() != 1
            || select.selection.is_none()
            || select.projection.is_empty()
            || select.projection.len() > 64
            || select.top.is_some()
            || select.select_modifiers.is_some()
            || select.exclude.is_some()
            || !select.optimizer_hints.is_empty()
            || !select.lateral_views.is_empty()
            || select.prewhere.is_some()
            || !select.connect_by.is_empty()
            || !select.cluster_by.is_empty()
            || !select.distribute_by.is_empty()
            || !select.sort_by.is_empty()
            || !select.named_window.is_empty()
            || select.qualify.is_some()
            || select.value_table_mode.is_some()
            || select.flavor != SelectFlavor::Standard
            || select.projection.iter().any(|item| {
                !matches!(
                    item,
                    SelectItem::UnnamedExpr(_) | SelectItem::ExprWithAlias { .. }
                )
            })
        {
            return ControlFlow::Break(());
        }
        if !matches!(&select.group_by, GroupByExpr::Expressions(_, modifiers) if modifiers.is_empty())
        {
            return ControlFlow::Break(());
        }
        for table in &select.from {
            if table.joins.len() > 3 {
                return ControlFlow::Break(());
            }
            for join in &table.joins {
                if !matches!(
                    &join.join_operator,
                    JoinOperator::Join(JoinConstraint::On(_))
                        | JoinOperator::Inner(JoinConstraint::On(_))
                        | JoinOperator::Left(JoinConstraint::On(_))
                        | JoinOperator::LeftOuter(JoinConstraint::On(_))
                ) {
                    return ControlFlow::Break(());
                }
            }
        }
        ControlFlow::Continue(())
    }

    fn pre_visit_table_factor(&mut self, table: &TableFactor) -> ControlFlow<()> {
        match table {
            TableFactor::Table {
                args,
                with_hints,
                version,
                with_ordinality,
                partitions,
                json_path,
                sample,
                index_hints,
                ..
            } if args.is_none()
                && with_hints.is_empty()
                && version.is_none()
                && !*with_ordinality
                && partitions.is_empty()
                && json_path.is_none()
                && sample.is_none()
                && index_hints.is_empty() =>
            {
                ControlFlow::Continue(())
            }
            _ => ControlFlow::Break(()),
        }
    }

    fn pre_visit_relation(&mut self, relation: &ObjectName) -> ControlFlow<()> {
        let name = relation.to_string();
        if name.split('.').count() > 2 || name.len() > 256 {
            return ControlFlow::Break(());
        }
        self.tables.insert(name.to_ascii_uppercase());
        ControlFlow::Continue(())
    }

    fn pre_visit_ident(&mut self, ident: &Ident) -> ControlFlow<()> {
        if ident.quote_style.is_some()
            || ident.value.len() > 128
            || ident.value.is_empty()
            || !ident
                .value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
            || ["NEXTVAL", "CURRVAL"].contains(&ident.value.to_ascii_uppercase().as_str())
        {
            return ControlFlow::Break(());
        }
        ControlFlow::Continue(())
    }

    fn pre_visit_value(&mut self, value: &ValueWithSpan) -> ControlFlow<()> {
        let Value::Placeholder(value) = &value.value else {
            return ControlFlow::Break(());
        };
        let Some(name) = value.strip_prefix(':') else {
            return ControlFlow::Break(());
        };
        if name.is_empty()
            || name.len() > 64
            || !name.as_bytes()[0].is_ascii_alphabetic()
            || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return ControlFlow::Break(());
        }
        self.parameters.insert(name.to_ascii_lowercase());
        ControlFlow::Continue(())
    }

    fn pre_visit_expr(&mut self, expression: &Expr) -> ControlFlow<()> {
        self.expression_depth += 1;
        self.expression_count += 1;
        if self.expression_depth > 48 || self.expression_count > 1024 {
            return ControlFlow::Break(());
        }
        match expression {
            Expr::Identifier(_)
            | Expr::CompoundIdentifier(_)
            | Expr::Nested(_)
            | Expr::Value(_)
            | Expr::IsNull(_)
            | Expr::IsNotNull(_) => (),
            Expr::BinaryOp {
                op:
                    BinaryOperator::Plus
                    | BinaryOperator::Minus
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide
                    | BinaryOperator::Eq
                    | BinaryOperator::NotEq
                    | BinaryOperator::Gt
                    | BinaryOperator::GtEq
                    | BinaryOperator::Lt
                    | BinaryOperator::LtEq
                    | BinaryOperator::And,
                ..
            } => (),
            Expr::UnaryOp {
                op: UnaryOperator::Plus | UnaryOperator::Minus,
                ..
            } => (),
            Expr::InList { negated, list, .. }
                if !*negated && !list.is_empty() && list.len() <= 1000 => {}
            Expr::Between { negated, .. } if !*negated => (),
            Expr::Function(function) => {
                let name = function.name.to_string().to_ascii_uppercase();
                if ![
                    "SUM", "COUNT", "MIN", "MAX", "AVG", "COALESCE", "ABS", "ROUND",
                ]
                .contains(&name.as_str())
                    || function.uses_odbc_syntax
                    || !matches!(&function.parameters, FunctionArguments::None)
                    || function.over.is_some()
                    || function.filter.is_some()
                    || function.null_treatment.is_some()
                    || !function.within_group.is_empty()
                {
                    return ControlFlow::Break(());
                }
                let FunctionArguments::List(arguments) = &function.args else {
                    return ControlFlow::Break(());
                };
                if arguments.args.is_empty()
                    || arguments.args.len() > 8
                    || !arguments.clauses.is_empty()
                {
                    return ControlFlow::Break(());
                }
                if arguments.args.iter().any(|argument| {
                    !matches!(argument, FunctionArg::Unnamed(FunctionArgExpr::Expr(_)))
                        && !(name == "COUNT"
                            && matches!(argument, FunctionArg::Unnamed(FunctionArgExpr::Wildcard)))
                }) {
                    return ControlFlow::Break(());
                }
                let valid_arity = match name.as_str() {
                    "COALESCE" => arguments.args.len() >= 2,
                    "ROUND" => arguments.args.len() <= 2,
                    _ => arguments.args.len() == 1,
                };
                if !valid_arity {
                    return ControlFlow::Break(());
                }
            }
            _ => return ControlFlow::Break(()),
        }
        ControlFlow::Continue(())
    }

    fn post_visit_expr(&mut self, _expression: &Expr) -> ControlFlow<()> {
        self.expression_depth -= 1;
        ControlFlow::Continue(())
    }
}

struct CanonicalNames;
impl VisitorMut for CanonicalNames {
    type Break = ();
    fn pre_visit_ident(&mut self, ident: &mut Ident) -> ControlFlow<()> {
        ident.value.make_ascii_uppercase();
        ControlFlow::Continue(())
    }
    fn pre_visit_value(&mut self, value: &mut ValueWithSpan) -> ControlFlow<()> {
        if let Value::Placeholder(value) = &mut value.value {
            value.make_ascii_lowercase();
        }
        ControlFlow::Continue(())
    }
}

#[cfg(test)]
#[path = "CratesRayportDomainSrcSqlInspectionTest.rs"]
mod tests;
