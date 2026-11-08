use crate::{
    inspect_sql, validate_apic_bind_guards, DomainError, Principal, QueryInput, SalesAggregate,
    SalesRow, SourceKind, Tool, ToolStatus,
};
use chrono::NaiveDate;
use std::collections::BTreeSet;

/// JSON consumers must be able to represent money without rounding.
pub const MAX_SAFE_MINOR: i64 = 9_007_199_254_740_991;

pub fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c))
        && !value.contains("..")
}

pub fn parse_date(value: &str) -> Result<NaiveDate, DomainError> {
    if value.len() != 10
        || value.as_bytes()[4] != b'-'
        || value.as_bytes()[7] != b'-'
        || value
            .bytes()
            .enumerate()
            .any(|(i, c)| i != 4 && i != 7 && !c.is_ascii_digit())
    {
        return Err(DomainError::new(
            "invalid_date",
            "날짜는 YYYY-MM-DD 형식이어야 합니다.",
        ));
    }
    let date = NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| DomainError::new("invalid_date", "유효한 날짜가 아닙니다."))?;
    if value.starts_with("0000") {
        return Err(DomainError::new("invalid_date", "0년은 지원하지 않습니다."));
    }
    Ok(date)
}

fn bounded_text(value: &str, limit: usize) -> bool {
    !value.trim().is_empty()
        && value.chars().count() <= limit
        && !value.chars().any(char::is_control)
}

/// Checks a draft's non-I/O invariants without implying publication or execution permission.
pub fn validate_tool_definition(tool: &Tool) -> Result<(), DomainError> {
    if !valid_identifier(&tool.id) || !valid_identifier(&tool.source_ref) || tool.version == 0 {
        return Err(DomainError::new(
            "invalid_tool",
            "도구 ID·원천 참조·버전을 확인하세요.",
        ));
    }
    if !bounded_text(&tool.name, 120)
        || !bounded_text(&tool.description, 2000)
        || !bounded_text(&tool.owner, 128)
        || tool.keywords.is_empty()
        || tool.keywords.len() > 20
        || tool.keywords.iter().any(|value| !bounded_text(value, 80))
    {
        return Err(DomainError::new(
            "invalid_tool",
            "도구 이름·설명·담당자·검색어를 확인하세요.",
        ));
    }
    if !(1..=366).contains(&tool.max_days)
        || !(1..=10_000).contains(&tool.max_rows)
        || tool.freshness_seconds == 0
        || tool.freshness_seconds > 31_536_000
    {
        return Err(DomainError::new(
            "invalid_policy",
            "조회 기간·행 수·최신성 정책이 허용 범위를 벗어났습니다.",
        ));
    }
    if let Some(sql) = tool.sql_template.as_deref() {
        inspect_sql(sql)?;
    }
    if tool.source == SourceKind::Apic {
        let sql = tool.sql_template.as_deref().ok_or_else(|| {
            DomainError::new(
                "sql_required",
                "APIC 도구에는 검증할 SQL 템플릿이 필요합니다.",
            )
        })?;
        let inspected = inspect_sql(sql)?;
        for required in ["start_date", "end_date_exclusive", "store_id"] {
            if !inspected
                .parameters
                .iter()
                .any(|parameter| parameter == required)
            {
                return Err(DomainError::new(
                    "missing_bind",
                    "APIC 템플릿에는 기간과 점포 바인드가 필요합니다.",
                ));
            }
        }
        if inspected.parameters.len() != 3 {
            return Err(DomainError::new(
                "unsupported_bind",
                "v1 조회 계약은 시작일·종료일·점포 외 입력을 지원하지 않습니다.",
            ));
        }
        validate_apic_bind_guards(sql)?;
    }
    Ok(())
}

/// Validation evidence records human/adapter verification; SQL syntax alone is insufficient.
pub fn validate_tool(tool: &Tool) -> Result<(), DomainError> {
    validate_tool_definition(tool)?;
    match tool.validation_evidence.as_deref() {
        Some(value) if bounded_text(value, 4000) => Ok(()),
        _ => Err(DomainError::new(
            "validation_required",
            "업무 의미·권한·정합성·부하 검증 근거가 필요합니다.",
        )),
    }
}

/// Pure transition rule. The application must additionally authorize the actor and audit it.
pub fn transition_tool(tool: &Tool, next: ToolStatus) -> Result<Tool, DomainError> {
    let allowed = matches!(
        (tool.status, next),
        (ToolStatus::Draft, ToolStatus::Validated)
            | (ToolStatus::Validated, ToolStatus::Published)
            | (ToolStatus::Validated, ToolStatus::Draft)
            | (ToolStatus::Published, ToolStatus::Retired)
    );
    if !allowed {
        return Err(DomainError::new(
            "invalid_transition",
            "허용되지 않은 도구 상태 변경입니다.",
        ));
    }
    if matches!(next, ToolStatus::Validated | ToolStatus::Published) {
        validate_tool(tool)?;
    }
    let mut updated = tool.clone();
    updated.status = next;
    if next == ToolStatus::Draft {
        updated.validation_evidence = None;
    }
    Ok(updated)
}

pub fn authorize_query(
    principal: &Principal,
    tool: &Tool,
    input: &QueryInput,
) -> Result<Vec<String>, DomainError> {
    if !valid_identifier(&principal.id) || !valid_identifier(&principal.tenant_id) {
        return Err(DomainError::new(
            "unauthorized",
            "검증된 사용자 신원이 필요합니다.",
        ));
    }
    if tool.status != ToolStatus::Published {
        return Err(DomainError::new(
            "not_published",
            "게시된 도구만 실행할 수 있습니다.",
        ));
    }
    validate_tool(tool)?;
    let start = parse_date(&input.start_date)?;
    let end = parse_date(&input.end_date_exclusive)?;
    let days = (end - start).num_days();
    if days <= 0 || days > i64::from(tool.max_days) {
        return Err(DomainError::new(
            "range_exceeded",
            "조회 기간이 도구의 허용 범위를 벗어났습니다.",
        ));
    }
    if input.store_ids.len() > 1000 || principal.store_ids.len() > 10_000 {
        return Err(DomainError::new(
            "scope_exceeded",
            "점포 범위가 허용 크기를 벗어났습니다.",
        ));
    }
    let authorized: BTreeSet<_> = principal.store_ids.iter().cloned().collect();
    if authorized.is_empty() || authorized.iter().any(|value| !valid_identifier(value)) {
        return Err(DomainError::new(
            "forbidden",
            "허용된 점포 범위가 없습니다.",
        ));
    }
    let requested = if input.store_ids.is_empty() {
        authorized.clone()
    } else {
        input.store_ids.iter().cloned().collect()
    };
    if requested.len() != input.store_ids.len() && !input.store_ids.is_empty() {
        return Err(DomainError::new(
            "duplicate_scope",
            "점포는 중복 지정할 수 없습니다.",
        ));
    }
    if !requested.is_subset(&authorized) {
        return Err(DomainError::new(
            "forbidden",
            "요청에 권한 밖 점포가 포함되어 있습니다.",
        ));
    }
    Ok(requested.into_iter().collect())
}

pub fn validate_sales_rows(rows: &[SalesRow]) -> Result<(), DomainError> {
    if rows.len() > 100_000 {
        return Err(DomainError::new(
            "rows_exceeded",
            "반입 행 수가 한도를 초과했습니다.",
        ));
    }
    let mut seen = BTreeSet::new();
    for row in rows {
        parse_date(&row.business_date)?;
        if !valid_identifier(&row.store_id) || row.currency != "KRW" {
            return Err(DomainError::new(
                "invalid_sales_row",
                "점포 ID 또는 통화가 유효하지 않습니다. v1은 KRW만 지원합니다.",
            ));
        }
        if !(0..=MAX_SAFE_MINOR).contains(&row.gross_minor)
            || !(0..=MAX_SAFE_MINOR).contains(&row.cancel_minor)
        {
            return Err(DomainError::new(
                "invalid_amount",
                "금액은 안전하게 표현 가능한 0 이상의 정수여야 합니다.",
            ));
        }
        if !seen.insert((&row.business_date, &row.store_id, &row.currency)) {
            return Err(DomainError::new(
                "duplicate_sales_row",
                "영업일·점포·통화의 중복 행이 있습니다.",
            ));
        }
    }
    Ok(())
}

pub fn aggregate_rows(
    rows: &[SalesRow],
    input: &QueryInput,
    scope: &[String],
) -> Result<SalesAggregate, DomainError> {
    validate_sales_rows(rows)?;
    let start = parse_date(&input.start_date)?;
    let end = parse_date(&input.end_date_exclusive)?;
    if end <= start {
        return Err(DomainError::new(
            "range_exceeded",
            "종료일은 시작일 이후여야 합니다.",
        ));
    }
    let allowed: BTreeSet<_> = scope.iter().collect();
    let mut selected: Vec<SalesRow> = rows
        .iter()
        .filter(|row| {
            row.business_date >= input.start_date
                && row.business_date < input.end_date_exclusive
                && allowed.contains(&row.store_id)
        })
        .cloned()
        .collect();
    if selected.is_empty() {
        return Err(DomainError::new(
            "no_data",
            "선택한 범위에 확보된 데이터가 없습니다. 실적 0과 다릅니다.",
        ));
    }
    selected.sort_by(|a, b| (&a.business_date, &a.store_id).cmp(&(&b.business_date, &b.store_id)));
    let mut gross_minor = 0i64;
    let mut cancel_minor = 0i64;
    for row in &selected {
        gross_minor = checked_amount_add(gross_minor, row.gross_minor)?;
        cancel_minor = checked_amount_add(cancel_minor, row.cancel_minor)?;
    }
    let net_minor = gross_minor
        .checked_sub(cancel_minor)
        .ok_or_else(amount_overflow)?;
    if !(-MAX_SAFE_MINOR..=MAX_SAFE_MINOR).contains(&net_minor) {
        return Err(amount_overflow());
    }
    Ok(SalesAggregate {
        rows: selected,
        gross_minor,
        cancel_minor,
        net_minor,
        currency: "KRW".into(),
    })
}

fn amount_overflow() -> DomainError {
    DomainError::new(
        "amount_overflow",
        "합계가 안전한 금액 표현 범위를 초과했습니다.",
    )
}
fn checked_amount_add(left: i64, right: i64) -> Result<i64, DomainError> {
    let value = left.checked_add(right).ok_or_else(amount_overflow)?;
    if value > MAX_SAFE_MINOR {
        return Err(amount_overflow());
    }
    Ok(value)
}

#[cfg(test)]
#[path = "CratesRayportDomainSrcPolicyTest.rs"]
mod tests;
