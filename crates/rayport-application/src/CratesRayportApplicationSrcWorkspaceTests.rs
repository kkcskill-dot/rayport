use crate::*;
use chrono::{DateTime, Duration, Utc};
use rayport_domain::*;

fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-11-08T10:00:00Z")
        .unwrap()
        .with_timezone(&Utc)
}
fn principal(role: Role, id: &str) -> Principal {
    Principal {
        id: id.into(),
        tenant_id: "tenant-one".into(),
        role,
        store_ids: vec!["S01".into()],
    }
}
fn admin() -> Principal {
    principal(Role::Admin, "admin01")
}
fn viewer() -> Principal {
    principal(Role::Viewer, "viewer01")
}
fn workspace() -> Workspace {
    Workspace::new("tenant-one".into())
}
fn query() -> QueryInput {
    QueryInput {
        start_date: "2026-11-07".into(),
        end_date_exclusive: "2026-11-08".into(),
        store_ids: vec!["S01".into()],
    }
}
fn row(date: &str, store: &str, gross: i64) -> SalesRow {
    SalesRow {
        business_date: date.into(),
        store_id: store.into(),
        gross_minor: gross,
        cancel_minor: 0,
        currency: "KRW".into(),
    }
}
fn mart(rows: Vec<SalesRow>) -> MartImport {
    MartImport {
        id: "sales-daily".into(),
        name: "일별 점포 실적".into(),
        data_as_of: "2026-11-08T09:30:00Z".into(),
        complete_through: "2026-11-07".into(),
        rows,
    }
}
fn tool(source: SourceKind) -> Tool {
    Tool {
        id: "sales.store_daily".into(),
        name: "점포 매출".into(),
        description: "영업일별 정정 반영 매출".into(),
        keywords: vec!["매출".into()],
        source,
        source_ref: "sales-daily".into(),
        sql_template: if source == SourceKind::Apic {
            Some("SELECT business_date, store_id, gross_minor, cancel_minor, currency FROM sales WHERE business_date >= :start_date AND business_date < :end_date_exclusive AND store_id = :store_id".into())
        } else {
            None
        },
        status: ToolStatus::Published,
        version: 1,
        max_days: 366,
        max_rows: 10000,
        freshness_seconds: 3600,
        owner: "sales-owner".into(),
        validation_evidence: Some("업무 정의·권한·정합성·실행 비용을 검증했습니다.".into()),
    }
}
fn discovery(sql: &str) -> DiscoveryImport {
    DiscoveryImport {
        window_start: None,
        window_end: None,
        source_label: "승인된 합성 관측자료".into(),
        diagnostics_authorized: true,
        observations: vec![Observation {
            sql_text: sql.into(),
            executions: None,
            ash_samples: Some(42),
            module: Some("매출 조회".into()),
        }],
    }
}
const TEMPLATE: &str = "SELECT amount FROM sales WHERE store_id = :store_id";

#[test]
fn tenant_separation_applies_to_read_and_write_paths() {
    let mut ws = workspace();
    let mut foreign = admin();
    foreign.tenant_id = "tenant-two".into();
    assert_eq!(ws.view(&foreign, false).unwrap_err().code, "forbidden");
    assert_eq!(ws.resolve(&foreign, "매출").unwrap_err().code, "forbidden");
    assert_eq!(
        ws.request(&foreign, "매출", "업무 확인", now())
            .unwrap_err()
            .code,
        "forbidden"
    );
    assert_eq!(
        ws.create_tool(&foreign, tool(SourceKind::Mart), now())
            .unwrap_err()
            .code,
        "forbidden"
    );
    assert_eq!(
        ws.reserve(&foreign, &tool(SourceKind::Mart), &query(), now())
            .unwrap_err()
            .code,
        "forbidden"
    );
    assert!(ws.tools.is_empty());
    assert!(ws.executions.is_empty());
}

#[test]
fn viewer_and_analyst_cannot_use_administrative_mutations() {
    for actor in [viewer(), principal(Role::Analyst, "analyst01")] {
        let mut ws = workspace();
        assert_eq!(
            ws.create_tool(&actor, tool(SourceKind::Mart), now())
                .unwrap_err()
                .code,
            "forbidden"
        );
        assert_eq!(
            ws.import_mart(&actor, mart(vec![row("2026-11-07", "S01", 1)]), now())
                .unwrap_err()
                .code,
            "forbidden"
        );
        assert_eq!(
            ws.import_discovery(&actor, discovery(TEMPLATE), now())
                .unwrap_err()
                .code,
            "forbidden"
        );
        assert_eq!(
            ws.policy(&actor, Policy::default(), now())
                .unwrap_err()
                .code,
            "forbidden"
        );
    }
}

#[test]
fn viewer_catalog_redacts_sql_evidence_and_other_users_requests() {
    let mut ws = workspace();
    let published = tool(SourceKind::Apic);
    ws.tools.push(published);
    let mut draft = tool(SourceKind::Mart);
    draft.id = "draft".into();
    draft.status = ToolStatus::Draft;
    ws.tools.push(draft);
    ws.request(&viewer(), "내 요청", "점포 업무", now())
        .unwrap();
    ws.request(
        &principal(Role::Viewer, "other"),
        "다른 사람 요청",
        "다른 업무",
        now(),
    )
    .unwrap();
    ws.import_discovery(&admin(), discovery(TEMPLATE), now())
        .unwrap();
    let view = ws.view(&viewer(), false).unwrap();
    assert_eq!(view.tools.len(), 1);
    assert!(view.tools[0].sql_template.is_none());
    assert!(view.tools[0].validation_evidence.is_none());
    assert!(view.candidates.is_empty());
    assert!(view.audit.is_empty());
    assert!(view.marts.is_empty());
    assert_eq!(view.requests.len(), 1);
    assert_eq!(view.requests[0].requester_id, viewer().id);
}

#[test]
fn client_cannot_create_a_published_tool_or_supply_validation_evidence() {
    let mut ws = workspace();
    let created = ws
        .create_tool(&admin(), tool(SourceKind::Mart), now())
        .unwrap();
    assert_eq!(created.status, ToolStatus::Draft);
    assert!(created.validation_evidence.is_none());
    assert_eq!(
        ws.transition(&admin(), &created.id, ToolStatus::Published, now())
            .unwrap_err()
            .code,
        "invalid_transition"
    );
    assert_eq!(
        ws.validate(
            &admin(),
            &created.id,
            "충분한 업무·권한 검증 근거입니다.",
            now()
        )
        .unwrap_err()
        .code,
        "missing_source"
    );
}

#[test]
fn publishing_mart_tool_requires_source_and_validation_then_status_transition() {
    let mut ws = workspace();
    ws.import_mart(&admin(), mart(vec![row("2026-11-07", "S01", 100)]), now())
        .unwrap();
    let draft = ws
        .create_tool(&admin(), tool(SourceKind::Mart), now())
        .unwrap();
    let validated = ws
        .validate(
            &admin(),
            &draft.id,
            "정합성·권한·업무 의미를 검증했습니다.",
            now(),
        )
        .unwrap();
    assert_eq!(validated.status, ToolStatus::Validated);
    assert_eq!(
        ws.transition(&admin(), &draft.id, ToolStatus::Published, now())
            .unwrap()
            .status,
        ToolStatus::Published
    );
}

#[test]
fn observation_import_requires_authorization_and_preserves_unknown_execution_count() {
    let mut ws = workspace();
    let mut batch = discovery(TEMPLATE);
    batch.diagnostics_authorized = false;
    assert_eq!(
        ws.import_discovery(&admin(), batch, now())
            .unwrap_err()
            .code,
        "diagnostics_authorization_required"
    );
    let imported = ws
        .import_discovery(&admin(), discovery(TEMPLATE), now())
        .unwrap();
    assert_eq!(imported[0].executions, None);
    assert_eq!(imported[0].ash_samples, Some(42));
    let repeated = ws
        .import_discovery(&admin(), discovery(&TEMPLATE.to_lowercase()), now())
        .unwrap();
    assert_eq!(repeated.len(), 1);
    assert_eq!(repeated[0].observation_version, 1);
    assert_eq!(ws.candidates.len(), 1);
    assert_eq!(ws.candidates.len(), 1);
}

#[test]
fn failed_observation_batch_is_rolled_back_by_clone_transaction_boundary() {
    let mut committed = workspace();
    let before = serde_json::to_value(&committed).unwrap();
    let mut input = discovery(TEMPLATE);
    input.observations.push(Observation {
        sql_text: "DELETE FROM sales".into(),
        executions: None,
        ash_samples: None,
        module: None,
    });
    let mut staged = committed.clone();
    let result = staged.import_discovery(&admin(), input, now());
    if result.is_ok() {
        committed = staged;
    }
    assert_eq!(result.unwrap_err().code, "unsafe_sql");
    assert_eq!(serde_json::to_value(committed).unwrap(), before);
}

#[test]
fn unsafe_raw_sql_is_not_persisted_in_a_draft() {
    let mut ws = workspace();
    let mut unsafe_tool = tool(SourceKind::Apic);
    unsafe_tool.sql_template =
        Some("SELECT amount FROM sales WHERE account = 'sensitive-value'".into());
    assert_eq!(
        ws.create_tool(&admin(), unsafe_tool, now())
            .unwrap_err()
            .code,
        "unsafe_sql"
    );
    assert!(ws.tools.is_empty());
    assert!(ws.audit.is_empty());
}

#[test]
fn a_candidate_can_create_one_draft_and_never_auto_publish() {
    let mut ws = workspace();
    let candidate = ws
        .import_discovery(&admin(), discovery(TEMPLATE), now())
        .unwrap()
        .remove(0);
    let draft = ws
        .candidate_draft(
            &admin(),
            &candidate.id,
            CandidateDraftCommand {
                name: "점포 매출".into(),
                source_ref: "sales-daily".into(),
                source: SourceKind::Mart,
                owner: "sales-owner".into(),
            },
            now(),
        )
        .unwrap();
    assert_eq!(draft.status, ToolStatus::Draft);
    assert!(draft.validation_evidence.is_none());
    assert_eq!(
        ws.candidate_draft(
            &admin(),
            &candidate.id,
            CandidateDraftCommand {
                name: "점포 매출".into(),
                source_ref: "sales-daily".into(),
                source: SourceKind::Mart,
                owner: "sales-owner".into()
            },
            now()
        )
        .unwrap_err()
        .code,
        "conflict"
    );
}

#[test]
fn requests_are_deduplicated_per_requester_only() {
    let mut ws = workspace();
    let first = ws
        .request(&viewer(), " 상품군 매출 ", "분석", now())
        .unwrap();
    let second = ws
        .request(&viewer(), "상품군 매출", "다시 요청", now())
        .unwrap();
    assert_eq!(first.id, second.id);
    assert_eq!(ws.requests.len(), 1);
    let other = ws
        .request(
            &principal(Role::Viewer, "other"),
            "상품군 매출",
            "분석",
            now(),
        )
        .unwrap();
    assert_ne!(first.id, other.id);
    assert_eq!(ws.requests.len(), 2);
}

#[test]
fn request_ready_requires_published_tool_and_only_requester_can_resolve() {
    let mut ws = workspace();
    let request = ws.request(&viewer(), "추가 매출", "업무", now()).unwrap();
    assert_eq!(
        ws.transition_request(&viewer(), &request.id, "in_review", None, now())
            .unwrap_err()
            .code,
        "forbidden"
    );
    ws.transition_request(&admin(), &request.id, "in_review", None, now())
        .unwrap();
    ws.transition_request(&admin(), &request.id, "building", None, now())
        .unwrap();
    assert_eq!(
        ws.transition_request(
            &admin(),
            &request.id,
            "ready",
            Some("missing".into()),
            now()
        )
        .unwrap_err()
        .code,
        "unpublished_tool"
    );
    ws.tools.push(tool(SourceKind::Mart));
    ws.transition_request(
        &admin(),
        &request.id,
        "ready",
        Some("sales.store_daily".into()),
        now(),
    )
    .unwrap();
    assert_eq!(
        ws.transition_request(&admin(), &request.id, "resolved", None, now())
            .unwrap_err()
            .code,
        "forbidden"
    );
    assert_eq!(
        ws.transition_request(
            &principal(Role::Viewer, "other"),
            &request.id,
            "resolved",
            None,
            now()
        )
        .unwrap_err()
        .code,
        "forbidden"
    );
    assert_eq!(
        ws.transition_request(&viewer(), &request.id, "resolved", None, now())
            .unwrap_err()
            .code,
        "resolution_not_verified"
    );
    let (execution, _) = ws
        .reserve(&viewer(), &tool(SourceKind::Mart), &query(), now())
        .unwrap();
    ws.finish(&execution, "completed").unwrap();
    // Explicit clock for the synthetic fixture: completion follows ready.
    ws.executions.last_mut().unwrap().completed_at_ms = Some(now().timestamp_millis() + 1);
    assert_eq!(
        ws.transition_request(&viewer(), &request.id, "resolved", None, now())
            .unwrap()
            .status,
        "resolved"
    );
}

#[test]
fn retired_linked_tool_cannot_complete_an_unresolved_request() {
    let mut ws = workspace();
    ws.tools.push(tool(SourceKind::Mart));
    let request = ws.request(&viewer(), "추가 매출", "업무", now()).unwrap();
    for state in ["in_review", "building", "ready"] {
        ws.transition_request(
            &admin(),
            &request.id,
            state,
            Some("sales.store_daily".into()),
            now(),
        )
        .unwrap();
    }
    ws.transition(&admin(), "sales.store_daily", ToolStatus::Retired, now())
        .unwrap();
    assert_eq!(
        ws.transition_request(&viewer(), &request.id, "resolved", None, now())
            .unwrap_err()
            .code,
        "unpublished_tool"
    );
}

#[test]
fn mart_requires_complete_date_store_grid_and_does_not_turn_missing_into_zero() {
    let mut ws = workspace();
    ws.import_mart(&admin(), mart(vec![row("2026-11-07", "S01", 0)]), now())
        .unwrap();
    let q = QueryInput {
        start_date: "2026-11-06".into(),
        ..query()
    };
    assert_eq!(
        ws.query_mart(&tool(SourceKind::Mart), &q, &["S01".into()], now())
            .unwrap_err()
            .code,
        "incomplete_data"
    );
    assert_eq!(
        ws.query_mart(&tool(SourceKind::Mart), &query(), &["S01".into()], now())
            .unwrap()
            .net_minor,
        0
    );
    assert_eq!(
        ws.query_mart(&tool(SourceKind::Mart), &query(), &["S02".into()], now())
            .unwrap_err()
            .code,
        "no_data"
    );
}

#[test]
fn stale_or_missing_marts_fail_without_creating_source_executions() {
    let mut ws = workspace();
    let t = tool(SourceKind::Mart);
    assert_eq!(
        ws.query_mart(&t, &query(), &["S01".into()], now())
            .unwrap_err()
            .code,
        "missing_source"
    );
    let mut input = mart(vec![row("2026-11-07", "S01", 100)]);
    input.data_as_of = "2026-11-07T09:00:00Z".into();
    ws.import_mart(&admin(), input, now()).unwrap();
    assert_eq!(
        ws.query_mart(&t, &query(), &["S01".into()], now())
            .unwrap_err()
            .code,
        "stale_data"
    );
    assert!(ws.executions.is_empty());
}

#[test]
fn mart_full_snapshot_replacement_removes_deleted_rows_and_keeps_last_complete_version() {
    let mut ws = workspace();
    ws.import_mart(
        &admin(),
        mart(vec![
            row("2026-11-07", "S01", 100),
            row("2026-11-07", "S02", 200),
        ]),
        now(),
    )
    .unwrap();
    let next = ws
        .import_mart(&admin(), mart(vec![row("2026-11-07", "S01", 90)]), now())
        .unwrap();
    assert_eq!(next.version, 2);
    assert_eq!(next.rows_count, 1);
    assert_eq!(ws.marts[0].versions[0].rows.len(), 2);
    assert_eq!(
        ws.query_mart(&tool(SourceKind::Mart), &query(), &["S02".into()], now())
            .unwrap_err()
            .code,
        "no_data"
    );
    assert_eq!(
        ws.query_mart(&tool(SourceKind::Mart), &query(), &["S01".into()], now())
            .unwrap()
            .net_minor,
        90
    );
}

#[test]
fn repeated_snapshot_never_adds_amounts_or_duplicates_rows() {
    let mut ws = workspace();
    for _ in 0..3 {
        ws.import_mart(&admin(), mart(vec![row("2026-11-07", "S01", 100)]), now())
            .unwrap();
    }
    let result = ws
        .query_mart(&tool(SourceKind::Mart), &query(), &["S01".into()], now())
        .unwrap();
    assert_eq!(result.net_minor, 100);
    assert_eq!(result.rows.len(), 1);
    assert_eq!(ws.marts[0].versions.len(), 1);
    assert_eq!(ws.marts[0].latest().version, 1);
}

#[test]
fn malformed_mart_import_leaves_previous_snapshot_unchanged() {
    let mut ws = workspace();
    ws.import_mart(&admin(), mart(vec![row("2026-11-07", "S01", 100)]), now())
        .unwrap();
    let before = serde_json::to_value(&ws).unwrap();
    let duplicate = row("2026-11-07", "S01", 1);
    assert_eq!(
        ws.import_mart(&admin(), mart(vec![duplicate.clone(), duplicate]), now())
            .unwrap_err()
            .code,
        "duplicate_sales_row"
    );
    assert_eq!(serde_json::to_value(&ws).unwrap(), before);
    let mut older = mart(vec![row("2026-11-07", "S01", 0)]);
    older.data_as_of = "2026-11-08T08:00:00Z".into();
    assert_eq!(
        ws.import_mart(&admin(), older, now()).unwrap_err().code,
        "stale_import"
    );
    let mut future = mart(vec![row("2026-11-07", "S01", 0)]);
    future.data_as_of = "2027-01-01T00:00:00Z".into();
    assert_eq!(
        ws.import_mart(&admin(), future, now()).unwrap_err().code,
        "invalid_timestamp"
    );
}

#[test]
fn result_row_limit_is_enforced_after_coverage_validation() {
    let mut ws = workspace();
    ws.import_mart(
        &admin(),
        mart(vec![
            row("2026-11-06", "S01", 100),
            row("2026-11-07", "S01", 100),
        ]),
        now(),
    )
    .unwrap();
    let mut t = tool(SourceKind::Mart);
    t.max_rows = 1;
    let q = QueryInput {
        start_date: "2026-11-06".into(),
        ..query()
    };
    assert_eq!(
        ws.query_mart(&t, &q, &["S01".into()], now())
            .unwrap_err()
            .code,
        "row_limit"
    );
}

#[test]
fn active_and_in_doubt_work_hold_slots_even_after_time_window() {
    let mut ws = workspace();
    ws.policy.max_concurrent = 1;
    let t = tool(SourceKind::Apic);
    let (execution, _) = ws.reserve(&viewer(), &t, &query(), now()).unwrap();
    ws.finish(&execution, "in_doubt").unwrap();
    assert_eq!(
        ws.reserve(
            &principal(Role::Viewer, "other"),
            &t,
            &query(),
            now() + Duration::days(2)
        )
        .unwrap_err()
        .code,
        "budget_exceeded"
    );
    assert_eq!(ws.executions[0].status, "in_doubt");
    ws.finish(&execution, "failed").unwrap();
    assert!(ws
        .reserve(&viewer(), &t, &query(), now() + Duration::days(2))
        .is_ok());
}

#[test]
fn per_user_and_source_budgets_cannot_be_reset_by_another_query() {
    let mut ws = workspace();
    ws.policy.per_user_per_minute = 1;
    let t = tool(SourceKind::Mart);
    let (execution, _) = ws.reserve(&viewer(), &t, &query(), now()).unwrap();
    ws.finish(&execution, "completed").unwrap();
    let mut other_query = query();
    other_query.start_date = "2026-11-06".into();
    assert_eq!(
        ws.reserve(&viewer(), &t, &other_query, now())
            .unwrap_err()
            .code,
        "budget_exceeded"
    );
    assert!(ws
        .reserve(&viewer(), &t, &other_query, now() + Duration::seconds(61))
        .is_ok());
    let mut ws = workspace();
    ws.policy.source_per_minute = 1;
    let (execution, _) = ws.reserve(&viewer(), &t, &query(), now()).unwrap();
    ws.finish(&execution, "completed").unwrap();
    assert_eq!(
        ws.reserve(&principal(Role::Viewer, "other"), &t, &query(), now())
            .unwrap_err()
            .code,
        "budget_exceeded"
    );
}

#[test]
fn apic_split_ranges_share_daily_budget_but_annual_mart_queries_do_not() {
    let mut ws = workspace();
    let t = tool(SourceKind::Apic);
    let first = QueryInput {
        start_date: "2026-01-01".into(),
        end_date_exclusive: "2026-02-01".into(),
        store_ids: vec!["S01".into()],
    };
    let (execution, _) = ws.reserve(&viewer(), &t, &first, now()).unwrap();
    ws.finish(&execution, "completed").unwrap();
    assert_eq!(
        ws.reserve(&viewer(), &t, &query(), now() + Duration::minutes(2))
            .unwrap_err()
            .code,
        "budget_exceeded"
    );
    let annual = QueryInput {
        start_date: "2025-01-01".into(),
        end_date_exclusive: "2026-01-01".into(),
        store_ids: vec!["S01".into()],
    };
    assert!(ws
        .reserve(
            &viewer(),
            &tool(SourceKind::Mart),
            &annual,
            now() + Duration::minutes(2)
        )
        .is_ok());
    assert!(ws
        .reserve(
            &viewer(),
            &t,
            &query(),
            now() + Duration::days(1) + Duration::seconds(1)
        )
        .is_ok());
}

#[test]
fn recovery_preserves_uncertain_apic_work_and_releases_local_mart_work() {
    let mut ws = workspace();
    ws.reserve(&viewer(), &tool(SourceKind::Apic), &query(), now())
        .unwrap();
    ws.reserve(&viewer(), &tool(SourceKind::Mart), &query(), now())
        .unwrap();
    ws.recover();
    assert_eq!(ws.executions[0].status, "in_doubt");
    assert_eq!(ws.executions[1].status, "failed");
    assert_eq!(
        ws.view(&admin(), true)
            .unwrap()
            .operations
            .active_executions,
        1
    );
}

#[test]
fn pause_and_invalid_policy_cannot_open_an_execution_path() {
    let mut ws = workspace();
    let invalid = Policy {
        max_concurrent: 0,
        ..Policy::default()
    };
    assert_eq!(
        ws.policy(&admin(), invalid, now()).unwrap_err().code,
        "invalid_policy"
    );
    let paused = Policy {
        paused: true,
        ..Policy::default()
    };
    ws.policy(&admin(), paused, now()).unwrap();
    assert_eq!(
        ws.reserve(&viewer(), &tool(SourceKind::Mart), &query(), now())
            .unwrap_err()
            .code,
        "budget_exceeded"
    );
    assert!(ws.executions.is_empty());
}

#[test]
fn question_resolution_cannot_invent_unsupported_analysis_or_choose_ambiguous_tool() {
    let mut ws = workspace();
    ws.tools.push(tool(SourceKind::Mart));
    assert_eq!(
        ws.resolve(&viewer(), "어제 매출").unwrap().status,
        "matched"
    );
    assert_eq!(
        ws.resolve(&viewer(), "상품군 매출").unwrap().status,
        "missing"
    );
    assert_eq!(
        ws.resolve(&viewer(), "매출 감소 원인").unwrap().status,
        "missing"
    );
    let mut other = tool(SourceKind::Mart);
    other.id = "other-sales".into();
    ws.tools.push(other);
    assert_eq!(
        ws.resolve(&viewer(), "어제 매출").unwrap().status,
        "ambiguous"
    );
}

#[test]
fn different_apic_api_ids_share_the_same_source_budget() {
    let mut ws = workspace();
    ws.policy.source_per_minute = 1;
    let mut first = tool(SourceKind::Apic);
    first.source_ref = "source-api-one".into();
    let mut second = first.clone();
    second.id = "sales.second".into();
    second.source_ref = "source-api-two".into();
    let (execution, _) = ws.reserve(&viewer(), &first, &query(), now()).unwrap();
    ws.finish(&execution, "completed").unwrap();
    assert_eq!(
        ws.reserve(&viewer(), &second, &query(), now())
            .unwrap_err()
            .code,
        "budget_exceeded"
    );
}
#[test]
fn source_identity_is_preserved_and_latest_statistics_replace_without_summing() {
    let mut ws = workspace();
    let mut first = discovery(TEMPLATE);
    first.source_label = "source-one".into();
    let a = ws
        .import_discovery(&admin(), first.clone(), now())
        .unwrap()
        .remove(0);
    let mut second = first.clone();
    second.source_label = "source-two".into();
    let b = ws
        .import_discovery(&admin(), second, now())
        .unwrap()
        .remove(0);
    assert_ne!(a.id, b.id);
    assert_eq!(ws.candidates.len(), 2);
    first.observations[0].executions = Some(7);
    first.observations[0].ash_samples = Some(3);
    let updated = ws
        .import_discovery(&admin(), first, now())
        .unwrap()
        .remove(0);
    assert_eq!(updated.id, a.id);
    assert_eq!(updated.executions, Some(7));
    assert_eq!(updated.ash_samples, Some(3));
    assert_eq!(updated.observation_version, 2);
    assert_eq!(ws.candidates.len(), 2);
}
#[test]
fn unsupported_original_question_cannot_be_marked_ready_with_an_unrelated_tool() {
    let mut ws = workspace();
    ws.tools.push(tool(SourceKind::Mart));
    let request = ws
        .request(&viewer(), "상품군별 매출", "확장 필요", now())
        .unwrap();
    ws.transition_request(&admin(), &request.id, "in_review", None, now())
        .unwrap();
    ws.transition_request(&admin(), &request.id, "building", None, now())
        .unwrap();
    assert_eq!(
        ws.transition_request(
            &admin(),
            &request.id,
            "ready",
            Some("sales.store_daily".into()),
            now()
        )
        .unwrap_err()
        .code,
        "capability_mismatch"
    );
    assert_eq!(ws.requests[0].status, "building");
    assert_eq!(
        ws.resolve(&viewer(), "상품군별 매출").unwrap().status,
        "missing"
    );
}
