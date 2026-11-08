use crate::workspace::fail;
use crate::*;
use chrono::{DateTime, Duration, Utc};
use rayport_domain::*;

impl Workspace {
    pub fn import_mart(
        &mut self,
        p: &Principal,
        mut input: MartImport,
        now: DateTime<Utc>,
    ) -> AppResult<MartSummary> {
        self.admin(p)?;
        if !valid_identifier(&input.id)
            || input.name.trim().is_empty()
            || input.name.len() > 200
            || input.rows.is_empty()
            || input.rows.len() > 20000
        {
            return Err(fail(
                "invalid_mart",
                "마트 ID·이름과 1~20,000개 행을 확인하세요.",
            ));
        }
        let asof = DateTime::parse_from_rfc3339(&input.data_as_of).map_err(|_| {
            fail(
                "invalid_timestamp",
                "올바른 ISO 데이터 기준 시각이 필요합니다.",
            )
        })?;
        if asof.with_timezone(&Utc) > now + Duration::minutes(5) {
            return Err(fail(
                "invalid_timestamp",
                "미래 시각의 데이터는 반입할 수 없습니다.",
            ));
        }
        let through = parse_date(&input.complete_through)?;
        validate_sales_rows(&input.rows)?;
        if input.rows.iter().any(|r| {
            parse_date(&r.business_date)
                .map(|d| d > through)
                .unwrap_or(true)
        }) {
            return Err(fail(
                "invalid_coverage",
                "완료 기준일보다 이후인 행이 있습니다.",
            ));
        }
        input.rows.sort_by(|a, b| {
            (&a.business_date, &a.store_id, &a.currency).cmp(&(
                &b.business_date,
                &b.store_id,
                &b.currency,
            ))
        });
        let old = self.marts.iter().find(|m| m.id == input.id);
        if let Some(m) = old {
            let v = m.latest();
            if m.name == input.name
                && v.data_as_of == asof.to_rfc3339()
                && v.complete_through == input.complete_through
                && v.rows == input.rows
            {
                return Ok(m.summary());
            }
        }
        if old.is_none() && self.marts.len() >= 30 {
            return Err(fail("capacity", "마트 개수 한도에 도달했습니다."));
        }
        if old.is_some_and(|m| {
            DateTime::parse_from_rfc3339(&m.latest().data_as_of).is_ok_and(|t| asof < t)
        }) {
            return Err(fail(
                "stale_import",
                "기존보다 오래된 데이터 기준 시각입니다.",
            ));
        }
        let other_rows: usize = self
            .marts
            .iter()
            .filter(|m| m.id != input.id)
            .map(|m| m.latest().rows.len())
            .sum();
        if other_rows + input.rows.len() > 100000 {
            return Err(fail("capacity", "단일 노드 마트 총행 한도에 도달했습니다."));
        }
        let version = old.map_or(1, |m| m.latest().version + 1);
        let v = MartVersion {
            version,
            data_as_of: asof.to_rfc3339(),
            complete_through: input.complete_through,
            rows: input.rows,
        };
        let summary = if let Some(m) = self.marts.iter_mut().find(|m| m.id == input.id) {
            m.name = input.name;
            m.versions.push(v);
            if m.versions.len() > 2 {
                m.versions.remove(0);
            }
            m.summary()
        } else {
            let m = Mart {
                id: input.id,
                name: input.name,
                versions: vec![v],
            };
            let summary = m.summary();
            self.marts.push(m);
            summary
        };
        self.log(p, "mart.snapshot_published", &summary.id, now);
        Ok(summary)
    }
    pub fn query_mart(
        &self,
        t: &Tool,
        q: &QueryInput,
        scope: &[String],
        now: DateTime<Utc>,
    ) -> AppResult<QueryResult> {
        let mart = self
            .marts
            .iter()
            .find(|m| m.id == t.source_ref)
            .ok_or_else(|| {
                fail(
                    "missing_source",
                    "등록된 마트가 없습니다. 원천으로 우회하지 않습니다.",
                )
            })?;
        let v = mart.latest();
        let asof = DateTime::parse_from_rfc3339(&v.data_as_of)
            .map_err(|_| fail("invalid_timestamp", "데이터 시각이 잘못되었습니다."))?;
        if now.signed_duration_since(asof).num_seconds() > t.freshness_seconds as i64 {
            return Err(fail(
                "stale_data",
                "마트의 허용 신선도를 초과했습니다. 갱신 후 다시 조회하세요.",
            ));
        }
        let start = parse_date(&q.start_date)?;
        let end = parse_date(&q.end_date_exclusive)?;
        if end.pred_opt().is_none_or(|last| {
            parse_date(&v.complete_through)
                .map(|through| last > through)
                .unwrap_or(true)
        }) {
            return Err(fail(
                "incomplete_data",
                "요청 기간의 자료가 아직 완성되지 않았습니다.",
            ));
        }
        let agg = aggregate_rows(&v.rows, q, scope)?;
        let expected = (end - start).num_days() as usize * scope.len();
        if agg.rows.len() != expected {
            return Err(fail(
                "incomplete_data",
                "요청한 날짜·점포의 일부 자료가 없습니다. 없는 자료를 0으로 처리하지 않습니다.",
            ));
        }
        if agg.rows.len() > t.max_rows {
            return Err(fail("row_limit", "결과 행 한도를 초과했습니다."));
        }
        Ok(QueryResult {
            rows: agg.rows,
            net_minor: agg.net_minor,
            gross_minor: agg.gross_minor,
            cancel_minor: agg.cancel_minor,
            currency: agg.currency,
            dataset_version: v.version,
            data_as_of: v.data_as_of.clone(),
            source: SourceKind::Mart,
        })
    }
}
