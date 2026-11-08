use crate::workspace::{fail, id};
use crate::*;
use chrono::{DateTime, Utc};
use rayport_domain::*;

impl Workspace {
    pub fn request(
        &mut self,
        p: &Principal,
        question: &str,
        purpose: &str,
        now: DateTime<Utc>,
    ) -> AppResult<CapabilityRequest> {
        self.authorize(p)?;
        if question.trim().is_empty() || question.len() > 4000 || purpose.len() > 2000 {
            return Err(fail("invalid_request", "질문과 목적의 길이를 확인하세요."));
        }
        if let Some(r) = self.requests.iter().find(|r| {
            r.requester_id == p.id
                && r.question.trim() == question.trim()
                && !matches!(r.status.as_str(), "resolved" | "rejected")
        }) {
            return Ok(r.clone());
        }
        if self.requests.len() >= 5000 {
            return Err(fail("capacity", "생성 요청 보관 한도에 도달했습니다."));
        }
        let r = CapabilityRequest {
            ready_after_execution_sequence: None,
            ready_at_ms: None,
            id: id("request"),
            question: question.trim().into(),
            purpose: purpose.trim().into(),
            status: "submitted".into(),
            requester_id: p.id.clone(),
            linked_tool_id: None,
            created_at: now.to_rfc3339(),
        };
        self.requests.push(r.clone());
        self.log(p, "request.created", &r.id, now);
        Ok(r)
    }
    pub fn transition_request(
        &mut self,
        p: &Principal,
        request_id: &str,
        status: &str,
        linked_tool_id: Option<String>,
        now: DateTime<Utc>,
    ) -> AppResult<CapabilityRequest> {
        self.authorize(p)?;
        let i = self
            .requests
            .iter()
            .position(|r| r.id == request_id)
            .ok_or_else(|| fail("not_found", "요청이 없습니다."))?;
        let r = &self.requests[i];
        if status == "resolved" {
            if r.requester_id != p.id {
                return Err(fail(
                    "forbidden",
                    "원래 요청자만 해결을 확인할 수 있습니다.",
                ));
            }
        } else {
            self.admin(p)?;
        }
        let allowed = matches!(
            (r.status.as_str(), status),
            ("submitted", "in_review")
                | ("in_review", "building")
                | ("building", "ready")
                | ("ready", "resolved")
                | ("ready", "building")
                | ("submitted", "rejected")
                | ("in_review", "rejected")
                | ("building", "rejected")
        );
        if !allowed {
            return Err(fail(
                "invalid_transition",
                "허용되지 않는 요청 상태 전환입니다.",
            ));
        }
        let linked = linked_tool_id.or_else(|| r.linked_tool_id.clone());
        if matches!(status, "ready" | "resolved")
            && !linked.as_ref().is_some_and(|id| {
                self.tools
                    .iter()
                    .any(|t| &t.id == id && t.status == ToolStatus::Published)
            })
        {
            return Err(fail("unpublished_tool", "게시된 도구를 연결해야 합니다."));
        }
        if matches!(status, "ready" | "resolved") {
            let resolution = self.resolve(p, &r.question)?;
            if resolution.status != "matched" || resolution.tool_id != linked {
                return Err(fail(
                    "capability_mismatch",
                    "연결한 도구가 원래 질문의 지원 범위를 충족하지 않습니다.",
                ));
            }
        }
        if status == "resolved"
            && !self.executions.iter().any(|e| {
                e.actor == p.id
                    && Some(&e.tool_id) == linked.as_ref()
                    && e.status == "completed"
                    && e.sequence > r.ready_after_execution_sequence.unwrap_or(u64::MAX)
            })
        {
            return Err(fail(
                "resolution_not_verified",
                "준비된 도구로 다시 조회해 결과를 확인한 후 해결 처리하세요.",
            ));
        }
        self.requests[i].status = status.into();
        self.requests[i].linked_tool_id = linked;
        if status == "ready" {
            self.requests[i].ready_after_execution_sequence = Some(self.next_execution_sequence);
        } else if status == "building" {
            self.requests[i].ready_after_execution_sequence = None;
        }
        self.requests[i].ready_at_ms = if status == "ready" {
            Some(now.timestamp_millis())
        } else if status == "building" {
            None
        } else {
            self.requests[i].ready_at_ms
        };
        let result = self.requests[i].clone();
        self.log(p, "request.status_changed", request_id, now);
        Ok(result)
    }
}
