use super::*;
use crate::Role;

fn tool() -> Tool {
    Tool {
        id: "sales.store_daily".into(),
        name: "점포 매출".into(),
        description: "정정 반영 순매출".into(),
        keywords: vec!["매출".into()],
        source: SourceKind::Mart,
        source_ref: "sales_daily".into(),
        sql_template: None,
        status: ToolStatus::Published,
        version: 1,
        max_days: 31,
        max_rows: 1000,
        freshness_seconds: 3600,
        owner: "sales-team".into(),
        validation_evidence: Some("정합성·권한·부하 확인".into()),
    }
}
fn principal() -> Principal {
    Principal {
        id: "user01".into(),
        tenant_id: "company".into(),
        role: Role::Viewer,
        store_ids: vec!["S01".into()],
    }
}
fn query() -> QueryInput {
    QueryInput {
        start_date: "2026-11-07".into(),
        end_date_exclusive: "2026-11-08".into(),
        store_ids: vec!["S01".into()],
    }
}
fn row(store: &str, gross: i64, cancel: i64) -> SalesRow {
    SalesRow {
        business_date: "2026-11-07".into(),
        store_id: store.into(),
        gross_minor: gross,
        cancel_minor: cancel,
        currency: "KRW".into(),
    }
}

#[test]
fn exact_dates_and_leap_days_are_validated() {
    assert!(parse_date("2024-02-29").is_ok());
    for invalid in [
        "2025-02-29",
        "2026-13-01",
        "2026-1-01",
        "2026-01-1",
        "2026-01-01Z",
        "0000-01-01",
        "2026-00-01",
        "2026-11-00",
    ] {
        assert_eq!(parse_date(invalid).unwrap_err().code, "invalid_date");
    }
}

#[test]
fn permissions_are_server_scope_and_never_role_bypass() {
    let mut input = query();
    input.store_ids.push("S02".into());
    assert_eq!(
        authorize_query(&principal(), &tool(), &input)
            .unwrap_err()
            .code,
        "forbidden"
    );
    let mut admin = principal();
    admin.role = Role::Admin;
    assert_eq!(
        authorize_query(&admin, &tool(), &input).unwrap_err().code,
        "forbidden"
    );
}

#[test]
fn omitted_scope_means_only_authorized_stores() {
    let mut input = query();
    input.store_ids.clear();
    assert_eq!(
        authorize_query(&principal(), &tool(), &input).unwrap(),
        vec!["S01"]
    );
    let mut user = principal();
    user.store_ids.clear();
    assert_eq!(
        authorize_query(&user, &tool(), &input).unwrap_err().code,
        "forbidden"
    );
}

#[test]
fn duplicate_scope_and_invalid_identity_are_rejected() {
    let mut input = query();
    input.store_ids.push("S01".into());
    assert_eq!(
        authorize_query(&principal(), &tool(), &input)
            .unwrap_err()
            .code,
        "duplicate_scope"
    );
    let mut user = principal();
    user.id.clear();
    assert_eq!(
        authorize_query(&user, &tool(), &query()).unwrap_err().code,
        "unauthorized"
    );
}

#[test]
fn dates_are_end_exclusive_and_bounded() {
    let mut input = query();
    input.end_date_exclusive = input.start_date.clone();
    assert_eq!(
        authorize_query(&principal(), &tool(), &input)
            .unwrap_err()
            .code,
        "range_exceeded"
    );
    input.start_date = "2026-01-01".into();
    input.end_date_exclusive = "2026-02-02".into();
    assert_eq!(
        authorize_query(&principal(), &tool(), &input)
            .unwrap_err()
            .code,
        "range_exceeded"
    );
    input.end_date_exclusive = "2026-02-01".into();
    assert!(authorize_query(&principal(), &tool(), &input).is_ok());
}

#[test]
fn published_status_does_not_bypass_validation() {
    let mut value = tool();
    value.validation_evidence = None;
    assert_eq!(
        authorize_query(&principal(), &value, &query())
            .unwrap_err()
            .code,
        "validation_required"
    );
    value = tool();
    value.max_rows = 0;
    assert_eq!(
        authorize_query(&principal(), &value, &query())
            .unwrap_err()
            .code,
        "invalid_policy"
    );
}

#[test]
fn drafts_and_retired_tools_cannot_execute() {
    for status in [
        ToolStatus::Draft,
        ToolStatus::Validated,
        ToolStatus::Retired,
    ] {
        let mut value = tool();
        value.status = status;
        assert_eq!(
            authorize_query(&principal(), &value, &query())
                .unwrap_err()
                .code,
            "not_published"
        );
    }
}

#[test]
fn draft_validation_does_not_require_evidence_but_transition_does() {
    let mut draft = tool();
    draft.status = ToolStatus::Draft;
    draft.validation_evidence = None;
    assert!(validate_tool_definition(&draft).is_ok());
    assert_eq!(
        transition_tool(&draft, ToolStatus::Validated)
            .unwrap_err()
            .code,
        "validation_required"
    );
    assert_eq!(
        transition_tool(&draft, ToolStatus::Published)
            .unwrap_err()
            .code,
        "invalid_transition"
    );
    draft.validation_evidence = Some("검증 완료".into());
    let checked = transition_tool(&draft, ToolStatus::Validated).unwrap();
    let published = transition_tool(&checked, ToolStatus::Published).unwrap();
    assert_eq!(published.status, ToolStatus::Published);
    let retired = transition_tool(&published, ToolStatus::Retired).unwrap();
    assert!(transition_tool(&retired, ToolStatus::Published).is_err());
    assert!(transition_tool(&checked, ToolStatus::Draft)
        .unwrap()
        .validation_evidence
        .is_none());
}

#[test]
fn source_reference_cannot_be_a_path_or_url() {
    for reference in [
        "../../file",
        "https://example.com",
        "x/y",
        "..",
        "a..b",
        "x?y",
    ] {
        let mut value = tool();
        value.source_ref = reference.into();
        assert_eq!(
            validate_tool_definition(&value).unwrap_err().code,
            "invalid_tool"
        );
    }
}

#[test]
fn apic_contract_requires_all_three_named_binds() {
    let mut value = tool();
    value.source = SourceKind::Apic;
    assert_eq!(
        validate_tool_definition(&value).unwrap_err().code,
        "sql_required"
    );
    value.sql_template = Some("SELECT amount FROM sales WHERE store_id = :store_id".into());
    assert_eq!(
        validate_tool_definition(&value).unwrap_err().code,
        "missing_bind"
    );
    value.sql_template = Some("SELECT amount FROM sales WHERE business_date >= :start_date AND business_date < :end_date_exclusive AND store_id = :store_id".into());
    assert!(validate_tool_definition(&value).is_ok());
    value
        .sql_template
        .as_mut()
        .unwrap()
        .push_str(" AND channel = :channel");
    assert_eq!(
        validate_tool_definition(&value).unwrap_err().code,
        "unsupported_bind"
    );
}

#[test]
fn aggregates_only_the_authorized_rows_and_keeps_dates() {
    let rows = vec![row("S01", 120000, 20000), row("S02", 80000, 5000)];
    let result = aggregate_rows(&rows, &query(), &["S01".into()]).unwrap();
    assert_eq!(result.net_minor, 100000);
    assert_eq!(result.rows.len(), 1);
    let total = aggregate_rows(&rows, &query(), &["S01".into(), "S02".into()]).unwrap();
    assert_eq!(total.net_minor, 175000);
}

#[test]
fn missing_data_is_not_reported_as_zero() {
    assert_eq!(
        aggregate_rows(&[], &query(), &["S01".into()])
            .unwrap_err()
            .code,
        "no_data"
    );
    let result = aggregate_rows(&[row("S01", 0, 0)], &query(), &["S01".into()]).unwrap();
    assert_eq!(result.net_minor, 0);
}

#[test]
fn end_date_is_excluded_from_aggregate() {
    let mut late = row("S01", 1, 0);
    late.business_date = "2026-11-08".into();
    assert_eq!(
        aggregate_rows(&[late], &query(), &["S01".into()])
            .unwrap_err()
            .code,
        "no_data"
    );
}

#[test]
fn duplicate_rows_are_rejected_instead_of_double_counted() {
    let value = row("S01", 100, 0);
    assert_eq!(
        validate_sales_rows(&[value.clone(), value])
            .unwrap_err()
            .code,
        "duplicate_sales_row"
    );
}

#[test]
fn currency_and_negative_components_are_rejected() {
    let mut value = row("S01", 100, 0);
    value.currency = "USD".into();
    assert_eq!(
        validate_sales_rows(&[value]).unwrap_err().code,
        "invalid_sales_row"
    );
    assert_eq!(
        validate_sales_rows(&[row("S01", -1, 0)]).unwrap_err().code,
        "invalid_amount"
    );
    assert_eq!(
        validate_sales_rows(&[row("S01", 0, -1)]).unwrap_err().code,
        "invalid_amount"
    );
}

#[test]
fn money_stays_exact_for_json_consumers_and_overflow_fails() {
    assert_eq!(
        validate_sales_rows(&[row("S01", i64::MAX, 0)])
            .unwrap_err()
            .code,
        "invalid_amount"
    );
    let rows = vec![row("S01", MAX_SAFE_MINOR, 0), row("S02", 1, 0)];
    assert_eq!(
        aggregate_rows(&rows, &query(), &["S01".into(), "S02".into()])
            .unwrap_err()
            .code,
        "amount_overflow"
    );
    assert_eq!(
        aggregate_rows(&[row("S01", 0, 100)], &query(), &["S01".into()])
            .unwrap()
            .net_minor,
        -100
    );
}
