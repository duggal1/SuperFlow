//! Scavenge claim/release — atomic first-to-claim work ownership.

use anyhow::Result;
use uuid::Uuid;

use crate::store::Store;

/// Result of attempting to claim a scavenge message.
pub enum ClaimResult {
    Claimed,
    AlreadyClaimed {
        claimed_by: String,
        claimed_at: String,
    },
    NotFound,
}

/// Attempt to claim a scavenge message. Only one worker can claim — first wins.
/// Updates the mail body_json in SQLite with claimed_by and claimed_at.
pub fn attempt_scavenge_claim(
    store: &Store,
    scavenge_mail_id: Uuid,
    claimer_session_id: Uuid,
    claimer_name: &str,
) -> Result<ClaimResult> {
    let rows = store.claim_scavenge_mail(scavenge_mail_id, claimer_session_id, claimer_name)?;
    if rows == 0 {
        // Check if already claimed or doesn't exist
        if let Some(body) = store.get_mail_body(scavenge_mail_id)? {
            let parsed: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
            if let Some(cb) = parsed.get("claimed_by").and_then(|v| v.as_str()) {
                let ca = parsed
                    .get("claimed_at")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                return Ok(ClaimResult::AlreadyClaimed {
                    claimed_by: cb.to_owned(),
                    claimed_at: ca.to_owned(),
                });
            }
            // Not a scavenge or no claim field
            if parsed.get("message_type").and_then(|v| v.as_str()) != Some("scavenge") {
                return Ok(ClaimResult::NotFound);
            }
        }
        return Ok(ClaimResult::NotFound);
    }
    Ok(ClaimResult::Claimed)
}

/// Release a claimed scavenge message back to the pool.
pub fn release_scavenge(store: &Store, scavenge_mail_id: Uuid, releaser_id: Uuid) -> Result<bool> {
    let rows = store.release_scavenge_mail(scavenge_mail_id, releaser_id)?;
    Ok(rows > 0)
}
