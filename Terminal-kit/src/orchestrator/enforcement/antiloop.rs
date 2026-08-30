use blake3::hash;

use crate::model::SessionState;
use crate::protocol::StatusDirective;

#[derive(Debug, Clone)]
pub struct StatusSignature {
    pub fingerprint: String,
}

pub struct AntiLoopOutcome {
    pub repeated_without_evidence: bool,
}

pub fn compute_status_signature(
    state: SessionState,
    directive: &StatusDirective,
) -> StatusSignature {
    let normalized = format!(
        "{}|{}|{}|{}",
        state.as_str(),
        directive
            .summary
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" "),
        directive.files.join(","),
        directive.commands.join(","),
    );
    StatusSignature {
        fingerprint: hash(normalized.as_bytes()).to_hex().to_string(),
    }
}

pub fn note_status_signature(
    last: &mut Option<StatusSignature>,
    repeat_count: &mut usize,
    current: StatusSignature,
    has_evidence: bool,
) -> AntiLoopOutcome {
    if let Some(prev) = last.as_ref()
        && prev.fingerprint == current.fingerprint
        && !has_evidence
    {
        *repeat_count = repeat_count.saturating_add(1);
    } else {
        *repeat_count = 0;
    }
    *last = Some(current);

    AntiLoopOutcome {
        repeated_without_evidence: *repeat_count >= 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeats_without_evidence_triggers() {
        let directive = StatusDirective {
            state: "progressing".to_owned(),
            summary: "I will do X".to_owned(),
            files: vec![],
            commands: vec![],
            risks: vec![],
            overlap: None,
        };
        let sig = compute_status_signature(SessionState::Progressing, &directive);
        let mut last = None;
        let mut repeats = 0;
        let out1 = note_status_signature(&mut last, &mut repeats, sig.clone(), false);
        assert!(!out1.repeated_without_evidence);
        let out2 = note_status_signature(&mut last, &mut repeats, sig.clone(), false);
        assert!(!out2.repeated_without_evidence);
        let out3 = note_status_signature(&mut last, &mut repeats, sig, false);
        assert!(out3.repeated_without_evidence);
    }
}
