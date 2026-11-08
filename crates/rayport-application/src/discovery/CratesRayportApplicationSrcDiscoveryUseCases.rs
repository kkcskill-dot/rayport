use crate::workspace::{fail, id};
use crate::*;
use chrono::{DateTime, Duration, Utc};
use rayport_domain::*;

impl Workspace {
    pub fn import_discovery(
        &mut self,
        p: &Principal,
        input: DiscoveryImport,
        now: DateTime<Utc>,
    ) -> AppResult<Vec<Candidate>> {
        self.admin(p)?;
        if !input.diagnostics_authorized {
            return Err(fail(
                "diagnostics_authorization_required",
                "AWR/ASH 사용권과 반입 권한을 확인해야 합니다.",
            ));
        }
        if input.source_label.trim().is_empty()
            || input.source_label.len() > 200
            || input.observations.is_empty()
            || input.observations.len() > 100
            || self.candidates.len() + input.observations.len() > 2000
        {
            return Err(fail(
                "invalid_import",
                "관측자료 출처와 1~100개 쿼리 템플릿을 지정하세요.",
            ));
        }
        match (&input.window_start, &input.window_end) {
            (Some(a), Some(b)) => {
                let start = DateTime::parse_from_rfc3339(a)
                    .map_err(|_| fail("invalid_window", "관측 시작 시각을 확인하세요."))?;
                let end = DateTime::parse_from_rfc3339(b)
                    .map_err(|_| fail("invalid_window", "관측 종료 시각을 확인하세요."))?;
                if start >= end || end.with_timezone(&Utc) > now + Duration::minutes(5) {
                    return Err(fail("invalid_window", "관측 기간을 확인하세요."));
                }
            }
            (None, None) => {}
            _ => return Err(fail("invalid_window", "관측 시작·종료를 함께 지정하세요.")),
        }
        // Store transaction is the all-or-nothing boundary. Latest observations replace, never sum overlapping windows.
        let mut imported = vec![];
        for o in input.observations {
            let inspected = inspect_sql(&o.sql_text)?;
            if o.module.as_ref().is_some_and(|m| m.len() > 200) {
                return Err(fail("invalid_import", "모듈명이 너무 깁니다."));
            }
            if let Some(existing) = self.candidates.iter_mut().find(|c| {
                c.signature == inspected.fingerprint && c.source_label == input.source_label.trim()
            }) {
                if existing.executions == o.executions
                    && existing.ash_samples == o.ash_samples
                    && existing.module == o.module
                    && existing.window_start == input.window_start
                    && existing.window_end == input.window_end
                {
                    imported.push(existing.clone());
                    continue;
                }
                existing.executions = o.executions;
                existing.ash_samples = o.ash_samples;
                existing.module = o.module;
                existing.window_start = input.window_start.clone();
                existing.window_end = input.window_end.clone();
                existing.observed_at = now.to_rfc3339();
                existing.observation_version += 1;
                imported.push(existing.clone());
                continue;
            }
            let c = Candidate {
                source_label: input.source_label.trim().into(),
                observed_at: now.to_rfc3339(),
                observation_version: 1,
                window_start: input.window_start.clone(),
                window_end: input.window_end.clone(),
                id: id("candidate"),
                name: o
                    .module
                    .clone()
                    .filter(|m| !m.trim().is_empty())
                    .unwrap_or_else(|| "업무 의미 확인이 필요한 조회".into()),
                sql_template: inspected.normalized_sql,
                signature: inspected.fingerprint,
                executions: o.executions,
                ash_samples: o.ash_samples,
                module: o.module,
                converted_tool_id: None,
            };
            self.candidates.push(c.clone());
            imported.push(c);
        }
        self.log(p, "discovery.imported", "observation-batch", now);
        Ok(imported)
    }
    pub fn candidate_draft(
        &mut self,
        p: &Principal,
        candidate_id: &str,
        input: CandidateDraftCommand,
        now: DateTime<Utc>,
    ) -> AppResult<Tool> {
        let CandidateDraftCommand {
            name,
            source_ref,
            source,
            owner,
        } = input;
        self.admin(p)?;
        let c = self
            .candidates
            .iter()
            .find(|c| c.id == candidate_id)
            .ok_or_else(|| fail("not_found", "후보가 없습니다."))?
            .clone();
        if c.converted_tool_id.is_some() {
            return Err(fail("conflict", "이미 도구 초안을 생성한 후보입니다."));
        }
        let t = Tool {
            id: id("tool"),
            name: name.clone(),
            description: "관측된 조회에서 생성한 초안입니다. 업무 정의와 실행 범위를 확인하세요."
                .into(),
            keywords: vec![name],
            source,
            source_ref,
            sql_template: Some(c.sql_template),
            status: ToolStatus::Draft,
            version: 1,
            max_days: 7,
            max_rows: 1000,
            freshness_seconds: 86400,
            owner,
            validation_evidence: None,
        };
        let tool = self.create_tool(p, t, now)?;
        self.candidates
            .iter_mut()
            .find(|c| c.id == candidate_id)
            .unwrap()
            .converted_tool_id = Some(tool.id.clone());
        Ok(tool)
    }
}
