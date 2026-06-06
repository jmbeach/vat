// @spec SYNC-ID-001, SYNC-ID-002, SYNC-ID-003, SYNC-ID-004, SYNC-ID-005, SYNC-ID-006

#![allow(dead_code)]

use std::collections::{HashMap, HashSet};

use thiserror::Error;

use crate::base32;

const MAX_RETRIES: usize = 100;

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum SyncIdError {
    // @spec SYNC-ID-006
    #[error("duplicate id [{id}] on lines {line1} and {line2}")]
    DuplicateId {
        id: String,
        line1: usize,
        line2: usize,
    },
    // @spec SYNC-ID-003
    #[error(
        "id generation exhausted {MAX_RETRIES} retries; project namespace may be full"
    )]
    RetryExhausted,
}

#[derive(Debug)]
pub(crate) struct AssignIdsResult {
    /// Parsed region with IDs inserted into previously-unid'd bullets.
    pub(crate) modified: String,
    /// Newly-assigned IDs to append to `.used-ids` *after* `backlog.md` is
    /// written successfully (SYNC-ID-004).
    pub(crate) new_ids: Vec<String>,
    /// Non-fatal warnings, e.g., foreign-prefix IDs (SYNC-ID-005).
    pub(crate) warnings: Vec<String>,
}

/// Assign IDs to every bullet in `parsed_region` that lacks one.
///
/// `tombstone_ids` is the pre-loaded set of IDs from `backlog/.used-ids`.
/// Collision avoidance is against `tombstone_ids` ∪ IDs already present in
/// `parsed_region` ∪ IDs assigned earlier in this call.
///
/// Pre-condition: `parsed_region` is LF-normalised (CRLF → LF handled by the
/// file-reading layer, see FMT-WS-001).
///
/// Returns `AssignIdsResult` on success, or:
/// - `SyncIdError::DuplicateId` when two bullets share the same `[id]` (SYNC-ID-006).
/// - `SyncIdError::RetryExhausted` when 100 collision-free candidates cannot be
///   found (SYNC-ID-003).
///
/// On either error, no files are modified — the function itself performs no I/O.
// @spec SYNC-ID-001, SYNC-ID-002, SYNC-ID-003, SYNC-ID-004, SYNC-ID-005, SYNC-ID-006
pub(crate) fn assign_ids(
    parsed_region: &str,
    project_id: &str,
    tombstone_ids: &HashSet<String>,
    rng: &mut impl rand::RngCore,
) -> Result<AssignIdsResult, SyncIdError> {
    let mut used: HashSet<String> = tombstone_ids.clone();
    let mut id_to_line: HashMap<String, usize> = HashMap::new();
    let mut warnings: Vec<String> = Vec::new();

    // Pass 1: collect existing IDs; detect duplicates and foreign prefixes.
    // @spec SYNC-ID-002, SYNC-ID-005, SYNC-ID-006
    for (idx, raw) in parsed_region.split_inclusive('\n').enumerate() {
        let line_no = idx + 1;
        let line = raw.trim_end_matches('\n');
        let Some(bullet_body) = line.strip_prefix("- ") else {
            continue;
        };
        let Some(id) = extract_bullet_id(bullet_body) else {
            continue;
        };
        let id_lower = id.to_ascii_lowercase();

        // SYNC-ID-006: duplicate id → hard error, no writes.
        if let Some(&prev) = id_to_line.get(&id_lower) {
            return Err(SyncIdError::DuplicateId {
                id: id_lower,
                line1: prev,
                line2: line_no,
            });
        }
        id_to_line.insert(id_lower.clone(), line_no);

        // SYNC-ID-005: foreign prefix → warn, leave unchanged (handled in pass 2).
        let prefix = id_lower.split_once('-').map(|(p, _)| p).unwrap_or("");
        if prefix != project_id {
            warnings.push(format!(
                "line {line_no}: id [{id_lower}] has foreign prefix \
                 {prefix:?} (project is {project_id:?}); left unchanged"
            ));
        }

        used.insert(id_lower);
    }

    // Pass 2: assign IDs to unid'd bullets; reconstruct the region text.
    // @spec SYNC-ID-001, SYNC-ID-002, SYNC-ID-003, SYNC-ID-004
    let mut new_ids: Vec<String> = Vec::new();
    let mut output = String::with_capacity(parsed_region.len() + 16);

    for raw in parsed_region.split_inclusive('\n') {
        let has_nl = raw.ends_with('\n');
        let line = raw.trim_end_matches('\n');

        if let Some(bullet_body) = line.strip_prefix("- ") {
            if bullet_body.trim().is_empty() {
                // Empty bullet: FMT-PARSE-006 says skip ID assignment; leave untouched.
                output.push_str(line);
            } else if extract_bullet_id(bullet_body).is_some() {
                // Already has an ID: pass through unchanged (includes foreign-prefix).
                output.push_str(line);
            } else {
                // No ID: generate one and prepend it.  @spec SYNC-ID-001
                let new_id = generate_id(project_id, &used, rng)?;
                // Record for tombstone append after backlog.md write.  @spec SYNC-ID-004
                new_ids.push(new_id.clone());
                // Update local collision set for subsequent bullets in this call.
                used.insert(new_id.clone());
                output.push_str("- [");
                output.push_str(&new_id);
                output.push_str("] ");
                output.push_str(bullet_body);
            }
        } else {
            output.push_str(line);
        }
        if has_nl {
            output.push('\n');
        }
    }

    Ok(AssignIdsResult {
        modified: output,
        new_ids,
        warnings,
    })
}

// @spec SYNC-ID-002, SYNC-ID-003
fn generate_id(
    project_id: &str,
    used: &HashSet<String>,
    rng: &mut impl rand::RngCore,
) -> Result<String, SyncIdError> {
    for _ in 0..MAX_RETRIES {
        let suffix = base32::random(3, rng);
        let candidate = format!("{project_id}-{suffix}");
        if !used.contains(&candidate) {
            return Ok(candidate);
        }
    }
    Err(SyncIdError::RetryExhausted)
}

/// Extract the `[id]` marker value from a bullet body (text after `"- "`).
///
/// Returns the raw ID token (without brackets) when the *first* `[...]`
/// token in `body` is a well-formed Crockford `<3>-<3>` ID.  Returns `None`
/// when the first token doesn't match (unknown marker, no bracket, etc.).
/// Casing is preserved; callers that need lowercase should call
/// `.to_ascii_lowercase()` on the result.
pub(crate) fn extract_bullet_id(body: &str) -> Option<&str> {
    let rest = body.strip_prefix('[')?;
    let end = rest.find(']')?;
    let candidate = &rest[..end];
    if is_valid_id_format(candidate) {
        Some(candidate)
    } else {
        None
    }
}

fn is_valid_id_format(s: &str) -> bool {
    match s.split_once('-') {
        Some((prefix, suffix)) => {
            base32::validate(prefix, 3).is_ok() && base32::validate(suffix, 3).is_ok()
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use rand::SeedableRng;

    use super::{MAX_RETRIES, SyncIdError, assign_ids, extract_bullet_id};
    use crate::base32;

    fn seeded() -> impl rand::RngCore {
        rand::rngs::StdRng::from_seed([0u8; 32])
    }

    fn no_ids() -> HashSet<String> {
        HashSet::new()
    }

    // ── extract_bullet_id ────────────────────────────────────────────────────

    #[test]
    fn extract_id_from_valid_marker() {
        assert_eq!(extract_bullet_id("[vat-s9g] title"), Some("vat-s9g"));
    }

    #[test]
    fn extract_id_preserves_original_casing() {
        assert_eq!(extract_bullet_id("[VAT-S9G] title"), Some("VAT-S9G"));
    }

    // "[in-progress]" has prefix "in" (2 chars) — fails 3-char validation.
    #[test]
    fn extract_id_returns_none_for_non_id_first_token() {
        assert_eq!(extract_bullet_id("[in-progress] title"), None);
    }

    #[test]
    fn extract_id_returns_none_for_no_bracket() {
        assert_eq!(extract_bullet_id("plain title"), None);
    }

    // "[abc]" has no "-" separator → not a valid id format.
    #[test]
    fn extract_id_returns_none_for_bracket_without_separator() {
        assert_eq!(extract_bullet_id("[abc] title"), None);
    }

    #[test]
    fn extract_id_returns_none_for_by_marker() {
        assert_eq!(extract_bullet_id("[by:jared] title"), None);
    }

    #[test]
    fn extract_id_returns_none_for_blocked_by_marker() {
        assert_eq!(extract_bullet_id("[blocked-by:vat-s9g] title"), None);
    }

    #[test]
    fn extract_id_returns_none_for_empty_brackets() {
        assert_eq!(extract_bullet_id("[] title"), None);
    }

    #[test]
    fn extract_id_returns_none_for_unclosed_bracket() {
        assert_eq!(extract_bullet_id("[vat-s9g title"), None);
    }

    // ── SYNC-ID-001 ──────────────────────────────────────────────────────────

    // @spec SYNC-ID-001
    #[test]
    fn bullet_without_id_gets_one_assigned() {
        let region = "- task title\n";
        let result = assign_ids(region, "vat", &no_ids(), &mut seeded()).unwrap();
        assert!(
            result.modified.starts_with("- [vat-"),
            "expected ID inserted: {:?}",
            result.modified
        );
    }

    // @spec SYNC-ID-001
    #[test]
    fn assigned_id_has_project_prefix() {
        let region = "- my task\n";
        let result = assign_ids(region, "vat", &no_ids(), &mut seeded()).unwrap();
        let line = result.modified.lines().next().unwrap_or("");
        assert!(line.starts_with("- [vat-"), "id should use project prefix: {line:?}");
    }

    // @spec SYNC-ID-001
    #[test]
    fn bullet_with_existing_id_is_unchanged() {
        // "vat-s9g": both segments are valid Crockford base32.
        let region = "- [vat-s9g] task title\n";
        let result = assign_ids(region, "vat", &no_ids(), &mut seeded()).unwrap();
        assert_eq!(result.modified, region);
    }

    // @spec SYNC-ID-001
    #[test]
    fn multiple_bullets_without_ids_all_get_ids() {
        let region = "- first task\n- second task\n- third task\n";
        // "abc" is a valid 3-char Crockford prefix (a, b, c are all in the alphabet).
        let result = assign_ids(region, "abc", &no_ids(), &mut seeded()).unwrap();
        let count = result
            .modified
            .lines()
            .filter(|l| l.contains("[abc-"))
            .count();
        assert_eq!(count, 3, "all three bullets should receive an ID");
        assert_eq!(result.new_ids.len(), 3);
    }

    // @spec SYNC-ID-001
    #[test]
    fn non_bullet_lines_are_preserved_unchanged() {
        let region = "# Heading\n\n- task\n  note line\n";
        let result = assign_ids(region, "vat", &no_ids(), &mut seeded()).unwrap();
        assert!(result.modified.contains("# Heading\n"));
        assert!(result.modified.contains("  note line\n"));
    }

    // @spec SYNC-ID-001
    #[test]
    fn empty_bullet_is_skipped_and_preserved() {
        let region = "- \n";
        let result = assign_ids(region, "vat", &no_ids(), &mut seeded()).unwrap();
        assert_eq!(result.modified, region, "empty bullet must be left untouched");
        assert!(result.new_ids.is_empty());
    }

    // @spec SYNC-ID-001
    #[test]
    fn mixed_id_d_and_unid_d_bullets_handled_correctly() {
        // "vat-abc": both segments valid Crockford (a, b, c ∈ alphabet).
        let region = "- [vat-abc] already has an id\n- needs an id\n";
        let result = assign_ids(region, "vat", &no_ids(), &mut seeded()).unwrap();
        let lines: Vec<&str> = result.modified.lines().collect();
        assert_eq!(lines[0], "- [vat-abc] already has an id", "existing ID unchanged");
        assert!(lines[1].starts_with("- [vat-"), "second bullet gets an ID");
        assert_eq!(result.new_ids.len(), 1, "only one new ID assigned");
    }

    // @spec SYNC-ID-001
    #[test]
    fn body_text_is_preserved_after_id_insertion() {
        let region = "- [in-progress] some title text\n";
        let result = assign_ids(region, "vat", &no_ids(), &mut seeded()).unwrap();
        let line = result.modified.lines().next().unwrap_or("");
        // The bullet had no ID (first token "[in-progress]" is not a Crockford id),
        // so an ID is prepended. The original body must follow.
        assert!(
            line.contains("[in-progress] some title text"),
            "original body must be preserved: {line:?}"
        );
    }

    // ── SYNC-ID-002 ──────────────────────────────────────────────────────────

    // @spec SYNC-ID-002
    #[test]
    fn generated_id_avoids_tombstone_collision() {
        // StdRng seed [0;32] generates "da2" as the first suffix (see base32 tests).
        let mut tombstone = HashSet::new();
        tombstone.insert("vat-da2".to_string());
        let region = "- task title\n";
        let result = assign_ids(region, "vat", &tombstone, &mut seeded()).unwrap();
        assert!(
            !result.modified.contains("[vat-da2]"),
            "must not reuse tombstone ID"
        );
        assert!(!result.new_ids.contains(&"vat-da2".to_string()));
    }

    // @spec SYNC-ID-002
    #[test]
    fn generated_id_avoids_existing_bullet_collision() {
        // Region has a bullet with [vat-da2]; a new bullet must not reuse it.
        let region = "- [vat-da2] first task\n- second task\n";
        let result = assign_ids(region, "vat", &no_ids(), &mut seeded()).unwrap();
        let second_line = result.modified.lines().nth(1).unwrap_or("");
        assert!(
            !second_line.contains("[vat-da2]"),
            "must not reuse existing bullet ID: {second_line:?}"
        );
    }

    // @spec SYNC-ID-002
    #[test]
    fn generated_ids_within_single_call_do_not_collide() {
        let region = "- first\n- second\n- third\n";
        let result = assign_ids(region, "vat", &no_ids(), &mut seeded()).unwrap();
        let unique: HashSet<_> = result.new_ids.iter().cloned().collect();
        assert_eq!(
            result.new_ids.len(),
            unique.len(),
            "all assigned IDs within one call must be unique"
        );
    }

    // ── SYNC-ID-003 ──────────────────────────────────────────────────────────

    // @spec SYNC-ID-003
    #[test]
    fn exhausted_retries_returns_error() {
        // Pre-generate the exact 100 IDs that the seeded RNG would produce,
        // then run assign_ids with the same seed — all 100 candidates are blocked.
        let mut seed_rng = rand::rngs::StdRng::from_seed([0u8; 32]);
        let mut used = HashSet::new();
        for _ in 0..MAX_RETRIES {
            let suffix = base32::random(3, &mut seed_rng);
            used.insert(format!("vat-{suffix}"));
        }
        let mut rng = rand::rngs::StdRng::from_seed([0u8; 32]);
        let err = assign_ids("- task\n", "vat", &used, &mut rng).unwrap_err();
        assert_eq!(err, SyncIdError::RetryExhausted);
    }

    // @spec SYNC-ID-003
    #[test]
    fn exhausted_retries_returns_err_not_partial_result() {
        // The function must return Err, not Ok with a partially-filled region.
        let mut seed_rng = rand::rngs::StdRng::from_seed([0u8; 32]);
        let mut used = HashSet::new();
        for _ in 0..MAX_RETRIES {
            used.insert(format!("vat-{}", base32::random(3, &mut seed_rng)));
        }
        let mut rng = rand::rngs::StdRng::from_seed([0u8; 32]);
        // Two bullets: if the first exhausts retries, an Err must be returned (not
        // a partial result with one bullet done and one skipped).
        let region = "- first task\n- second task\n";
        let err = assign_ids(region, "vat", &used, &mut rng).unwrap_err();
        assert_eq!(err, SyncIdError::RetryExhausted);
    }

    // ── SYNC-ID-004 ──────────────────────────────────────────────────────────

    // @spec SYNC-ID-004
    #[test]
    fn new_ids_returned_for_tombstone_append() {
        let region = "- first\n- second\n";
        let result = assign_ids(region, "vat", &no_ids(), &mut seeded()).unwrap();
        assert_eq!(result.new_ids.len(), 2, "one new ID per unid'd bullet");
        // Each new ID must appear in the modified region.
        for id in &result.new_ids {
            assert!(
                result.modified.contains(id.as_str()),
                "new id {id:?} must appear in modified region"
            );
        }
    }

    // @spec SYNC-ID-004
    #[test]
    fn already_id_d_bullets_produce_no_new_id_entries() {
        // "vat-abc": valid Crockford (a, b, c all in alphabet).
        let region = "- [vat-abc] already has an id\n";
        let result = assign_ids(region, "vat", &no_ids(), &mut seeded()).unwrap();
        assert!(
            result.new_ids.is_empty(),
            "no new IDs for bullets that already have one"
        );
    }

    // ── SYNC-ID-005 ──────────────────────────────────────────────────────────

    // @spec SYNC-ID-005
    #[test]
    fn foreign_prefix_bullet_produces_warning_and_is_unchanged() {
        // "bar-abc": "bar" and "abc" are both valid Crockford (b, a, r, c all in alphabet).
        let region = "- [bar-abc] a bullet from another project\n";
        let result = assign_ids(region, "vat", &no_ids(), &mut seeded()).unwrap();
        assert_eq!(result.modified, region, "foreign-prefix bullet must be unchanged");
        assert!(!result.warnings.is_empty(), "a warning must be emitted");
        assert!(
            result.warnings[0].contains("bar"),
            "warning should name the foreign prefix: {:?}",
            result.warnings[0]
        );
    }

    // @spec SYNC-ID-005
    #[test]
    fn foreign_prefix_id_not_added_to_new_ids() {
        let region = "- [bar-abc] bullet\n";
        let result = assign_ids(region, "vat", &no_ids(), &mut seeded()).unwrap();
        assert!(
            result.new_ids.is_empty(),
            "foreign-prefix IDs must not appear in new_ids"
        );
    }

    // @spec SYNC-ID-005
    #[test]
    fn warning_names_the_line_number() {
        let region = "preamble\n- [bar-abc] foreign bullet\n";
        let result = assign_ids(region, "vat", &no_ids(), &mut seeded()).unwrap();
        assert!(!result.warnings.is_empty());
        assert!(
            result.warnings[0].contains("line 2"),
            "warning must name the 1-based line number: {:?}",
            result.warnings[0]
        );
    }

    // @spec SYNC-ID-005
    #[test]
    fn foreign_prefix_id_enters_collision_set() {
        // A bullet [vat-abc] in a "vat" project is NOT foreign — it's the project's own id.
        // Verify it's tracked in the collision set so a second bullet doesn't reuse it.
        let region = "- [vat-abc] existing\n- needs id\n";
        let result = assign_ids(region, "vat", &no_ids(), &mut seeded()).unwrap();
        let second_line = result.modified.lines().nth(1).unwrap_or("");
        assert!(
            !second_line.contains("[vat-abc]"),
            "must not reassign an existing id: {second_line:?}"
        );
    }

    // ── SYNC-ID-006 ──────────────────────────────────────────────────────────

    // @spec SYNC-ID-006
    #[test]
    fn duplicate_id_in_region_returns_error() {
        // "vat-abc": both segments are valid Crockford base32.
        let region = "- [vat-abc] first\n- [vat-abc] second\n";
        let err = assign_ids(region, "vat", &no_ids(), &mut seeded()).unwrap_err();
        assert!(
            matches!(
                &err,
                SyncIdError::DuplicateId { id, line1: 1, line2: 2 } if id == "vat-abc"
            ),
            "expected DuplicateId, got {err:?}"
        );
    }

    // @spec SYNC-ID-006
    #[test]
    fn duplicate_id_error_names_both_line_numbers() {
        let region = "preamble\n- [vat-s9g] first\n- [vat-s9g] second\n";
        let err = assign_ids(region, "vat", &no_ids(), &mut seeded()).unwrap_err();
        match err {
            SyncIdError::DuplicateId { line1, line2, .. } => {
                assert_eq!(line1, 2);
                assert_eq!(line2, 3);
            }
            _ => panic!("expected DuplicateId, got {err:?}"),
        }
    }

    // @spec SYNC-ID-006
    #[test]
    fn duplicate_id_detection_is_case_insensitive() {
        // "VAT-ABC" and "vat-abc": V/v, A/a, T/t, A/a, B/b, C/c all in Crockford alphabet.
        let region = "- [VAT-ABC] first\n- [vat-abc] second\n";
        let err = assign_ids(region, "vat", &no_ids(), &mut seeded()).unwrap_err();
        assert!(
            matches!(err, SyncIdError::DuplicateId { .. }),
            "mixed-case duplicate must be detected"
        );
    }

    // @spec SYNC-ID-006
    #[test]
    fn duplicate_id_error_reports_normalised_lowercase_id() {
        let region = "- [VAT-ABC] first\n- [vat-abc] second\n";
        let err = assign_ids(region, "vat", &no_ids(), &mut seeded()).unwrap_err();
        match err {
            SyncIdError::DuplicateId { id, .. } => {
                assert_eq!(id, "vat-abc", "id in error must be lowercase-normalised");
            }
            _ => panic!("expected DuplicateId"),
        }
    }

    // @spec SYNC-ID-006
    #[test]
    fn no_error_when_ids_are_all_distinct() {
        // All IDs use valid Crockford segments: abc, def, ghj are each 3 valid chars.
        let region = "- [vat-abc] first\n- [vat-def] second\n- [vat-ghj] third\n";
        let result = assign_ids(region, "vat", &no_ids(), &mut seeded()).unwrap();
        assert_eq!(result.modified, region, "no-dup region must pass through unchanged");
    }
}
