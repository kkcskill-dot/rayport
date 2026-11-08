use crate::workspace::{fail, id, is_active};
use crate::*;
use chrono::{DateTime, Utc};
use rayport_domain::*;

impl Workspace {
    pub fn resolve(&self, p: &Principal, question: &str) -> AppResult<Resolution> {
        self.authorize(p)?;
        if question.trim().is_empty() || question.len() > 4000 {
            return Err(fail(
                "invalid_question",
                "질문을 1~4,000자 범위로 입력하세요.",
            ));
        }
        let q = question.to_lowercase();
        let unsupported = [
            "상품군",
            "고객별",
            "원인",
            "예측",
            "고유 고객",
            "재고",
            "채널별",
        ];
        let tools: Vec<_> = self
            .tools
            .iter()
            .filter(|t| t.status == ToolStatus::Published)
            .collect();
        let matches: Vec<_> = tools
            .iter()
            .filter(|t| {
                t.keywords
                    .iter()
                    .any(|k| !k.is_empty() && q.contains(&k.to_lowercase()))
            })
            .collect();
        if unsupported.iter().any(|s| q.contains(s)) {
            return Ok(Resolution {
                status: "missing".into(),
                tool_id: None,
                explanation:
                    "1.0 매출 도구가 제공하지 않는 분석 차원입니다. 기능 확장을 요청할 수 있습니다."
                        .into(),
                suggestions: vec![],
            });
        }
        match matches.as_slice(){[t]=>Ok(Resolution{status:"matched".into(),tool_id:Some(t.id.clone()),explanation:"등록된 검색어로 도구를 찾았습니다. 기간과 점포를 확인한 뒤 조회하세요. 자유 SQL이나 AI 추론은 실행하지 않습니다.".into(),suggestions:vec![t.name.clone()]}),[]=>Ok(Resolution{status:"missing".into(),tool_id:None,explanation:"지원하는 게시 도구를 찾지 못했습니다. 기존 도구를 확인하거나 생성 요청을 등록하세요.".into(),suggestions:tools.iter().take(5).map(|t|t.name.clone()).collect()}),_=>Ok(Resolution{status:"ambiguous".into(),tool_id:None,explanation:"여러 도구가 일치합니다. 업무 API에서 적절한 도구를 선택하세요.".into(),suggestions:matches.iter().take(5).map(|t|t.name.clone()).collect()})}
    }
    pub fn reserve(
        &mut self,
        p: &Principal,
        t: &Tool,
        q: &QueryInput,
        now: DateTime<Utc>,
    ) -> AppResult<(String, Vec<String>)> {
        self.authorize(p)?;
        let scope = authorize_query(p, t, q)?;
        if t.source == SourceKind::Apic && scope.len() != 1 {
            return Err(fail(
                "invalid_scope",
                "APIC 1.0 실행은 점포 하나로 제한됩니다.",
            ));
        }
        self.executions
            .retain(|e| is_active(&e.status) || e.started_at > now.timestamp() - 86400);
        let active = self
            .executions
            .iter()
            .filter(|e| is_active(&e.status))
            .count();
        let user_count = self
            .executions
            .iter()
            .filter(|e| e.actor == p.id && e.started_at > now.timestamp() - 60)
            .count();
        let source_count = self
            .executions
            .iter()
            .filter(|e| {
                e.source == t.source
                    && (t.source == SourceKind::Apic || e.source_ref == t.source_ref)
                    && e.started_at > now.timestamp() - 60
            })
            .count();
        let days = (parse_date(&q.end_date_exclusive)? - parse_date(&q.start_date)?).num_days();
        let used_days: i64 = self
            .executions
            .iter()
            .filter(|e| {
                e.actor == p.id
                    && e.source == SourceKind::Apic
                    && e.started_at > now.timestamp() - 86400
            })
            .map(|e| e.charged_days)
            .sum();
        if self.policy.paused
            || active >= self.policy.max_concurrent
            || user_count >= self.policy.per_user_per_minute
            || source_count >= self.policy.source_per_minute
            || (t.source == SourceKind::Apic && used_days + days > 31)
        {
            return Err(fail(
                "budget_exceeded",
                "운영 보호 정책으로 실행할 수 없습니다. 범위를 확인하거나 잠시 후 재시도하세요.",
            ));
        }
        self.next_execution_sequence = self
            .next_execution_sequence
            .checked_add(1)
            .ok_or_else(|| fail("capacity", "실행 번호 한도입니다."))?;
        let execution_id = id("execution");
        self.executions.push(Execution {
            sequence: self.next_execution_sequence,
            completed_at_ms: None,
            id: execution_id.clone(),
            actor: p.id.clone(),
            tool_id: t.id.clone(),
            source_ref: t.source_ref.clone(),
            source: t.source,
            status: "running".into(),
            started_at: now.timestamp(),
            charged_days: days,
        });
        Ok((execution_id, scope))
    }
    pub fn finish(&mut self, execution_id: &str, status: &str) -> AppResult<()> {
        if !matches!(status, "completed" | "failed" | "in_doubt") {
            return Err(fail("invalid_transition", "잘못된 실행 상태입니다."));
        }
        let e = self
            .executions
            .iter_mut()
            .find(|e| e.id == execution_id)
            .ok_or_else(|| fail("not_found", "실행 기록이 없습니다."))?;
        if !is_active(&e.status) {
            return Err(fail("conflict", "종료된 실행입니다."));
        }
        e.status = status.into();
        if status != "in_doubt" {
            e.completed_at_ms = Some(Utc::now().timestamp_millis());
        }
        if status == "completed" {
            self.completed_executions += 1;
        }
        Ok(())
    }
    pub fn recover(&mut self) {
        for e in &mut self.executions {
            if e.status == "running" {
                e.status = if e.source == SourceKind::Apic {
                    "in_doubt"
                } else {
                    "failed"
                }
                .into();
            }
        }
    }
}
