use crate::protocol::StatusDirective;

/// Evidence check for completion claims.
///
/// We keep this strict: a done claim must carry at least one of:
/// - touched files
/// - commands run
pub fn evidence_missing_for_done_claim(directive: &StatusDirective) -> bool {
    directive.files.is_empty() && directive.commands.is_empty()
}
