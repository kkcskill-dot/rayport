use crate::workspace::{fail, id};
use crate::*;
use chrono::{DateTime, Utc};
use rayport_domain::*;

impl Workspace {
    pub fn authorize(&self, p: &Principal) -> AppResult<()> {
        if p.tenant_id != self.tenant_id {
            Err(fail("forbidden", "다른 업무 공간에 접근할 수 없습니다."))
        } else {
            Ok(())
        }
    }
    pub fn admin(&self, p: &Principal) -> AppResult<()> {
        self.authorize(p)?;
        if p.role != Role::Admin {
            Err(fail("forbidden", "관리자 권한이 필요합니다."))
        } else {
            Ok(())
        }
    }
    pub(crate) fn log(&mut self, p: &Principal, action: &str, object: &str, now: DateTime<Utc>) {
        self.audit.push(AuditEvent {
            id: id("audit"),
            actor: p.id.clone(),
            action: action.into(),
            object_id: object.into(),
            at: now.to_rfc3339(),
        });
        if self.audit.len() > 2000 {
            self.audit.drain(..self.audit.len() - 2000);
        }
    }
    pub fn policy(
        &mut self,
        p: &Principal,
        policy: Policy,
        now: DateTime<Utc>,
    ) -> AppResult<Policy> {
        self.admin(p)?;
        if !(1..=100).contains(&policy.per_user_per_minute)
            || !(1..=1000).contains(&policy.source_per_minute)
            || !(1..=16).contains(&policy.max_concurrent)
        {
            return Err(fail(
                "invalid_policy",
                "호출 한도와 동시 실행 제한을 확인하세요.",
            ));
        }
        self.policy = policy.clone();
        self.log(p, "policy.changed", "workspace", now);
        Ok(policy)
    }
}
