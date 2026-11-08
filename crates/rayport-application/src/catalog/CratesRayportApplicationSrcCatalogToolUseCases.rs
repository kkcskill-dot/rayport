use crate::workspace::{fail, id};
use crate::*;
use chrono::{DateTime, Utc};
use rayport_domain::*;

impl Workspace {
    pub fn create_tool(
        &mut self,
        p: &Principal,
        mut tool: Tool,
        now: DateTime<Utc>,
    ) -> AppResult<Tool> {
        self.admin(p)?;
        if self.tools.len() >= 1000 {
            return Err(fail("capacity", "도구 등록 한도에 도달했습니다."));
        }
        if tool.id.is_empty() {
            tool.id = id("tool");
        }
        if !valid_identifier(&tool.id)
            || tool.name.trim().is_empty()
            || tool.name.len() > 200
            || tool.description.len() > 4000
            || tool.keywords.len() > 20
            || tool
                .keywords
                .iter()
                .any(|k| k.trim().is_empty() || k.len() > 100)
        {
            return Err(fail(
                "invalid_tool",
                "도구 이름·설명·검색어·ID를 확인하세요.",
            ));
        }
        if self.tools.iter().any(|t| t.id == tool.id) {
            return Err(fail("conflict", "같은 도구 ID가 있습니다."));
        }
        tool.status = ToolStatus::Draft;
        tool.version = 1;
        tool.validation_evidence = None;
        // Persist only approved SQL syntax, including drafts; never retain unsafe raw statements.
        if let Some(sql) = &tool.sql_template {
            tool.sql_template = Some(inspect_sql(sql)?.normalized_sql);
        }
        self.log(p, "tool.created", &tool.id, now);
        self.tools.push(tool.clone());
        Ok(tool)
    }
    pub fn edit_tool(
        &mut self,
        p: &Principal,
        tool_id: &str,
        mut next: Tool,
        now: DateTime<Utc>,
    ) -> AppResult<Tool> {
        self.admin(p)?;
        let i = self
            .tools
            .iter()
            .position(|t| t.id == tool_id)
            .ok_or_else(|| fail("not_found", "도구를 찾을 수 없습니다."))?;
        let old = &self.tools[i];
        if !matches!(old.status, ToolStatus::Draft | ToolStatus::Validated)
            || next.version != old.version
        {
            return Err(fail(
                "conflict",
                "편집 가능한 최신 초안이 아닙니다. 새로고침 후 확인하세요.",
            ));
        }
        if next.id != tool_id {
            return Err(fail("invalid_tool", "도구 ID는 변경할 수 없습니다."));
        }
        next.status = ToolStatus::Draft;
        next.validation_evidence = None;
        next.version = old
            .version
            .checked_add(1)
            .ok_or_else(|| fail("capacity", "도구 버전 한도를 초과했습니다."))?;
        if let Some(sql) = &next.sql_template {
            next.sql_template = Some(inspect_sql(sql)?.normalized_sql);
        }
        validate_tool_definition(&next)?;
        self.tools[i] = next.clone();
        self.log(p, "tool.edited", tool_id, now);
        Ok(next)
    }
    pub fn validate(
        &mut self,
        p: &Principal,
        tool_id: &str,
        evidence: &str,
        now: DateTime<Utc>,
    ) -> AppResult<Tool> {
        self.admin(p)?;
        if evidence.trim().len() < 10 || evidence.len() > 4000 {
            return Err(fail(
                "invalid_evidence",
                "업무 정의·권한·정합성 검증 근거를 10자 이상 입력하세요.",
            ));
        }
        let idx = self
            .tools
            .iter()
            .position(|t| t.id == tool_id)
            .ok_or_else(|| fail("not_found", "도구를 찾을 수 없습니다."))?;
        let mut tool = self.tools[idx].clone();
        tool.validation_evidence = Some(evidence.trim().into());
        if tool.source == SourceKind::Mart && !self.marts.iter().any(|m| m.id == tool.source_ref) {
            return Err(fail("missing_source", "먼저 해당 마트를 반입하세요."));
        }
        validate_tool(&tool)?;
        tool = transition_tool(&tool, ToolStatus::Validated)?;
        self.tools[idx] = tool.clone();
        self.log(p, "tool.validated", tool_id, now);
        Ok(tool)
    }
    pub fn transition(
        &mut self,
        p: &Principal,
        tool_id: &str,
        status: ToolStatus,
        now: DateTime<Utc>,
    ) -> AppResult<Tool> {
        self.admin(p)?;
        let t = self
            .tools
            .iter_mut()
            .find(|t| t.id == tool_id)
            .ok_or_else(|| fail("not_found", "도구를 찾을 수 없습니다."))?;
        let next = transition_tool(t, status)?;
        *t = next.clone();
        self.log(p, "tool.status_changed", tool_id, now);
        Ok(next)
    }
    pub fn tool(&self, id: &str) -> AppResult<Tool> {
        self.tools
            .iter()
            .find(|t| t.id == id)
            .cloned()
            .ok_or_else(|| fail("not_found", "도구를 찾을 수 없습니다."))
    }
}
