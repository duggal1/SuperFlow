use super::*;

impl Orchestrator {
    pub fn render_status(&self) -> Result<String> {
        let sessions = self.store.list_sessions()?;
        let active: Vec<_> = sessions
            .iter()
            .filter(|s| s.status == "running" || s.status == "launching")
            .collect();
        let completed: Vec<_> = sessions
            .iter()
            .filter(|s| s.status == "completed")
            .collect();
        let failed: Vec<_> = sessions.iter().filter(|s| s.status == "failed").collect();

        let mut lines = Vec::new();
        lines.push(format!("{} total missions", sessions.len()));
        lines.push(String::new());

        if !active.is_empty() {
            lines.push("── active ──".to_owned());
            for s in &active {
                lines.push(format!(
                    "  {}  {}  {}",
                    truncate_id(&s.id),
                    pad_status(&s.status),
                    s.mission_rewrite
                ));
            }
            lines.push(String::new());
        }

        if !completed.is_empty() {
            lines.push("── completed ──".to_owned());
            for s in &completed {
                lines.push(format!(
                    "  {}  {}  {}",
                    truncate_id(&s.id),
                    pad_status(&s.status),
                    s.mission_rewrite
                ));
            }
            lines.push(String::new());
        }

        if !failed.is_empty() {
            lines.push("── failed ──".to_owned());
            for s in &failed {
                lines.push(format!(
                    "  {}  {}  {}",
                    truncate_id(&s.id),
                    pad_status(&s.status),
                    s.mission_rewrite
                ));
            }
            lines.push(String::new());
        }

        if lines.len() <= 1 {
            lines.push(
                "no missions yet — launch one with: sp <agent> <count> \"mission\"".to_owned(),
            );
        }

        Ok(lines.join("\n"))
    }

    pub fn render_sessions(&self) -> Result<String> {
        let sessions = self.store.list_sessions()?;
        let mut lines = Vec::new();
        for session in sessions {
            lines.push(format!(
                "{} | {} | {} | {}",
                session.id,
                session.status,
                session.repo_path.display(),
                session.mission_rewrite
            ));
            if let Some(summary) = session.final_summary {
                lines.push(format!("final: {summary}"));
            }
        }
        if lines.is_empty() {
            lines.push("no sessions found".to_owned());
        }
        Ok(lines.join("\n"))
    }

    pub fn render_replay(&self, mission_id: Uuid, limit: usize) -> Result<String> {
        let snapshot = self
            .store
            .load_mission_snapshot(mission_id)?
            .context("unknown session id")?;
        let entries = self.store.recent_replay_entries(mission_id, limit)?;
        let mut lines = vec![
            format!("session: {}", snapshot.id),
            format!("status: {}", snapshot.status),
            format!("mission: {}", snapshot.mission_rewrite),
        ];
        if let Some(summary) = snapshot.final_summary {
            lines.push(format!("final_summary: {summary}"));
        }
        lines.push(String::new());
        for entry in entries {
            lines.push(format!(
                "{} | {} | {} | {}",
                entry.created_at.to_rfc3339(),
                entry.lane,
                entry.kind,
                truncate(&entry.body, 240)
            ));
        }
        Ok(lines.join("\n"))
    }

    #[allow(dead_code)]
    pub fn render_worker_status(&self, mission_id: Uuid) -> Result<String> {
        let workers = self.store.load_workers(mission_id)?;
        let mut lines = Vec::new();
        for worker in workers
            .iter()
            .filter(|worker| worker.session.role == SessionRole::Worker)
        {
            lines.push(format!(
                "{} [{}] {}",
                worker.session.name,
                worker.session.status.as_str(),
                worker
                    .session
                    .last_summary
                    .as_deref()
                    .unwrap_or("no summary")
            ));
        }
        if lines.is_empty() {
            lines.push("no workers found".to_owned());
        }
        Ok(lines.join("\n"))
    }

    pub fn render_worker_replay(
        &self,
        mission_id: Uuid,
        worker: &str,
        limit: usize,
    ) -> Result<String> {
        let workers = self.store.load_workers(mission_id)?;
        let target = workers
            .into_iter()
            .find(|candidate| {
                candidate.session.id.to_string() == worker
                    || candidate.session.name == worker
                    || candidate
                        .packet
                        .as_ref()
                        .map(|packet| packet.worker_id.as_str() == worker)
                        .unwrap_or(false)
            })
            .context("unknown worker for session")?;
        let entries = self
            .store
            .recent_worker_replay(mission_id, target.session.id, limit)?;
        let mut lines = vec![format!(
            "{} [{}]",
            target.session.name,
            target.session.status.as_str()
        )];
        for entry in entries {
            lines.push(format!(
                "{} | {} | {}",
                entry.created_at.to_rfc3339(),
                entry.kind,
                truncate(&entry.body, 240)
            ));
        }
        Ok(lines.join("\n"))
    }

    pub fn render_supervisor_summary(&self, mission_id: Uuid) -> Result<String> {
        let summary = self
            .store
            .latest_supervisor_summary(mission_id)?
            .context("no supervisor summary found")?;
        Ok(summary)
    }
}
