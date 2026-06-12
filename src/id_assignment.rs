// @spec SYNC-ID-001, SYNC-ID-002, SYNC-ID-003, SYNC-ID-005, SYNC-ID-006

use std::collections::HashSet;

use thiserror::Error;

use crate::base32;

const MAX_RETRIES: usize = 100;

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum IdAssignmentError {
    // @spec SYNC-ID-006
    #[error("duplicate id in parsed region: {0}")]
    DuplicateId(String),
    // @spec SYNC-ID-003
    #[error(
        "ID generation exceeded {MAX_RETRIES} retries for project prefix {0}; namespace may be full"
    )]
    RetryExhausted(String),
}

/// Assigns IDs to task entries that lack them.
///
/// `entry_ids` is one slot per task entry: `None` if the entry has no [id] yet,
/// `Some(id)` if it already does. On success the `None` slots are filled in with
/// newly-generated IDs.
///
/// `used` must be pre-populated with all tombstone IDs and all IDs currently present in
/// the parsed region. Newly-generated IDs are added to `used` as they are assigned so
/// that retries within the same run avoid self-collision.
///
/// Returns `(new_ids, warnings)`:
/// - `new_ids`: the newly-assigned IDs, in entry order (for the caller to append to
///   `.used-ids` after a successful write of `backlog.md`).
/// - `warnings`: human-readable messages for bullets whose existing ID has a project
///   prefix that differs from `project_id` (SYNC-ID-005).
///
/// On error the function may have partially mutated `entry_ids` and `used`.
/// The caller must not write any file when this function returns `Err`.
// @spec SYNC-ID-001, SYNC-ID-002, SYNC-ID-003, SYNC-ID-005, SYNC-ID-006
pub(crate) fn assign_ids(
    entry_ids: &mut [Option<String>],
    used: &mut HashSet<String>,
    project_id: &str,
    rng: &mut impl rand::RngCore,
) -> Result<(Vec<String>, Vec<String>), IdAssignmentError> {
    // SYNC-ID-006: detect duplicate IDs before making any assignments.
    let mut seen: HashSet<&str> = HashSet::new();
    for id in entry_ids.iter().filter_map(|slot| slot.as_deref()) {
        if !seen.insert(id) {
            return Err(IdAssignmentError::DuplicateId(id.to_owned()));
        }
    }

    // SYNC-ID-002 (region clause): a generated candidate must not collide with an ID
    // "currently present on another bullet in the parsed region." That clause is enforced
    // here solely via the `used` set: the caller must pre-seed `used` with every existing
    // parsed-region ID (and all tombstones). This debug_assert documents and, in debug
    // builds, enforces that contract so a caller that forgets to seed an existing bullet's
    // ID fails loudly here rather than silently minting a colliding ID. The covering
    // integration tests live in `sync::tests`.
    debug_assert!(
        entry_ids
            .iter()
            .filter_map(|slot| slot.as_deref())
            .all(|id| used.contains(id)),
        "caller must seed `used` with all existing entry IDs before calling assign_ids \
         (SYNC-ID-002 region clause)"
    );

    let mut new_ids: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    for slot in entry_ids.iter_mut() {
        if let Some(existing_id) = slot {
            // SYNC-ID-005: warn when the existing ID's prefix doesn't match.
            // `split('-').next()` yields the whole string for a dash-less or otherwise
            // malformed id (e.g. "abcdef" or ""), so such ids deliberately fall into this
            // same foreign-prefix branch: they are warned about and passed through
            // unchanged rather than treated as a hard error. This matches the LLD's
            // "warn, but pass through" intent for non-matching prefixes; we intentionally
            // do not distinguish a structurally-malformed id from a foreign-project id.
            let prefix = existing_id.split('-').next().unwrap_or("");
            if prefix != project_id {
                warnings.push(format!(
                    "warning: id {existing_id:?} has prefix {prefix:?}, expected {project_id:?}; leaving unchanged"
                ));
            }
        } else {
            // SYNC-ID-001, SYNC-ID-002, SYNC-ID-003: generate a fresh ID.
            let mut found: Option<String> = None;
            for _ in 0..MAX_RETRIES {
                let suffix = base32::random(3, rng);
                let candidate = format!("{project_id}-{suffix}");
                if used.insert(candidate.clone()) {
                    found = Some(candidate);
                    break;
                }
            }
            let id =
                found.ok_or_else(|| IdAssignmentError::RetryExhausted(project_id.to_owned()))?;
            new_ids.push(id.clone());
            *slot = Some(id);
        }
    }

    Ok((new_ids, warnings))
}

#[cfg(test)]
mod tests {
    use super::{IdAssignmentError, MAX_RETRIES, assign_ids};
    use rand::SeedableRng;
    use std::collections::HashSet;

    fn seeded_rng() -> impl rand::RngCore {
        rand::rngs::StdRng::from_seed([0u8; 32])
    }

    // @spec SYNC-ID-001
    #[test]
    fn assigns_id_to_entry_without_one() {
        let mut ids = vec![None];
        let mut used = HashSet::new();
        let mut rng = seeded_rng();
        let (new_ids, warnings) = assign_ids(&mut ids, &mut used, "foo", &mut rng).unwrap();
        assert_eq!(new_ids.len(), 1);
        assert!(warnings.is_empty());
        assert!(ids[0].is_some());
        let id = ids[0].as_ref().unwrap();
        assert!(id.starts_with("foo-"), "id {id:?} should start with foo-");
    }

    // @spec SYNC-ID-001
    #[test]
    fn assigned_id_has_correct_structure() {
        let mut ids = vec![None];
        let mut used = HashSet::new();
        let mut rng = seeded_rng();
        let (new_ids, _) = assign_ids(&mut ids, &mut used, "vat", &mut rng).unwrap();
        let id = &new_ids[0];
        let parts: Vec<&str> = id.splitn(2, '-').collect();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0], "vat");
        assert_eq!(parts[1].len(), 3);
    }

    // @spec SYNC-ID-001
    #[test]
    fn assigns_ids_to_multiple_entries_without_ids() {
        let mut ids = vec![None, None, None];
        let mut used = HashSet::new();
        let mut rng = seeded_rng();
        let (new_ids, warnings) = assign_ids(&mut ids, &mut used, "foo", &mut rng).unwrap();
        assert_eq!(new_ids.len(), 3);
        assert!(warnings.is_empty());
        assert!(ids.iter().all(Option::is_some));
        // All assigned IDs must be distinct.
        let id_set: HashSet<&str> = new_ids.iter().map(String::as_str).collect();
        assert_eq!(id_set.len(), 3);
    }

    // @spec SYNC-ID-001
    #[test]
    fn skips_entries_that_already_have_ids() {
        let mut ids = vec![Some("foo-abc".to_owned()), None, Some("foo-def".to_owned())];
        let mut used: HashSet<String> = ["foo-abc".to_owned(), "foo-def".to_owned()]
            .into_iter()
            .collect();
        let mut rng = seeded_rng();
        let (new_ids, _) = assign_ids(&mut ids, &mut used, "foo", &mut rng).unwrap();
        assert_eq!(new_ids.len(), 1);
        assert_eq!(ids[0].as_deref(), Some("foo-abc"));
        assert_eq!(ids[2].as_deref(), Some("foo-def"));
        assert!(ids[1].is_some());
    }

    // @spec SYNC-ID-001
    #[test]
    fn empty_entry_list_is_a_noop() {
        let mut ids: Vec<Option<String>> = vec![];
        let mut used = HashSet::new();
        let mut rng = seeded_rng();
        let (new_ids, warnings) = assign_ids(&mut ids, &mut used, "foo", &mut rng).unwrap();
        assert!(new_ids.is_empty());
        assert!(warnings.is_empty());
    }

    // @spec SYNC-ID-001
    #[test]
    fn new_ids_are_added_to_used_set() {
        let mut ids = vec![None];
        let mut used = HashSet::new();
        let mut rng = seeded_rng();
        assign_ids(&mut ids, &mut used, "foo", &mut rng).unwrap();
        let id = ids[0].as_ref().unwrap();
        assert!(
            used.contains(id.as_str()),
            "used set should contain the new id {id:?}"
        );
    }

    // @spec SYNC-ID-002
    #[test]
    fn skips_candidates_already_in_used_set() {
        // Force a collision on the first candidate by pre-seeding `used`.
        // We know seed [0;32] produces "da2" as the first 3 chars; seed that candidate.
        let mut ids = vec![None];
        let mut rng = seeded_rng();
        // Peek at what the first candidate would be.
        let first_suffix = {
            use rand::SeedableRng;
            let mut r = rand::rngs::StdRng::from_seed([0u8; 32]);
            crate::base32::random(3, &mut r)
        };
        let first_candidate = format!("vat-{first_suffix}");
        let mut used: HashSet<String> = [first_candidate.clone()].into_iter().collect();
        let (new_ids, _) = assign_ids(&mut ids, &mut used, "vat", &mut rng).unwrap();
        assert_ne!(
            new_ids[0], first_candidate,
            "should have skipped the pre-used candidate"
        );
        assert!(new_ids[0].starts_with("vat-"));
    }

    // @spec SYNC-ID-002
    #[test]
    fn candidates_are_unique_across_entries_in_same_run() {
        let mut ids = vec![None, None, None, None, None];
        let mut used = HashSet::new();
        let mut rng = seeded_rng();
        let (new_ids, _) = assign_ids(&mut ids, &mut used, "foo", &mut rng).unwrap();
        let id_set: HashSet<&str> = new_ids.iter().map(String::as_str).collect();
        assert_eq!(
            id_set.len(),
            new_ids.len(),
            "no duplicate IDs assigned in same run"
        );
    }

    // @spec SYNC-ID-003
    #[test]
    fn returns_retry_exhausted_error_when_namespace_is_full() {
        let alphabet: &[u8] = b"0123456789abcdefghjkmnpqrstvwxyz";
        let mut used: HashSet<String> = HashSet::new();
        // Fill every possible 3-char suffix for prefix "vat".
        for a in alphabet {
            for b in alphabet {
                for c in alphabet {
                    used.insert(format!("vat-{}{}{}", *a as char, *b as char, *c as char));
                }
            }
        }
        let mut ids = vec![None];
        let mut rng = seeded_rng();
        let err = assign_ids(&mut ids, &mut used, "vat", &mut rng).unwrap_err();
        assert_eq!(err, IdAssignmentError::RetryExhausted("vat".to_owned()));
    }

    // @spec SYNC-ID-003
    #[test]
    fn retry_exhausted_message_names_the_prefix() {
        let mut ids = vec![None];
        // Fill just enough to exhaust MAX_RETRIES retries.
        let alphabet: &[u8] = b"0123456789abcdefghjkmnpqrstvwxyz";
        let mut used: HashSet<String> = HashSet::new();
        for a in alphabet {
            for b in alphabet {
                for c in alphabet {
                    used.insert(format!("abc-{}{}{}", *a as char, *b as char, *c as char));
                }
            }
        }
        let mut rng = seeded_rng();
        let err = assign_ids(&mut ids, &mut used, "abc", &mut rng).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("abc"),
            "error message should name the prefix; got: {msg:?}"
        );
        assert!(
            msg.contains(&MAX_RETRIES.to_string()),
            "error message should name the retry cap; got: {msg:?}"
        );
    }

    // @spec SYNC-ID-005
    #[test]
    fn warns_on_foreign_prefix() {
        let mut ids = vec![Some("bar-abc".to_owned())];
        let mut used: HashSet<String> = ["bar-abc".to_owned()].into_iter().collect();
        let mut rng = seeded_rng();
        let (new_ids, warnings) = assign_ids(&mut ids, &mut used, "foo", &mut rng).unwrap();
        assert!(new_ids.is_empty());
        assert_eq!(warnings.len(), 1);
        assert!(
            warnings[0].contains("bar-abc"),
            "warning should mention the offending id: {warnings:?}"
        );
    }

    // @spec SYNC-ID-005
    #[test]
    fn foreign_prefix_entry_is_left_unchanged() {
        let mut ids = vec![Some("bar-abc".to_owned())];
        let mut used: HashSet<String> = ["bar-abc".to_owned()].into_iter().collect();
        let mut rng = seeded_rng();
        assign_ids(&mut ids, &mut used, "foo", &mut rng).unwrap();
        assert_eq!(ids[0].as_deref(), Some("bar-abc"));
    }

    // @spec SYNC-ID-005
    #[test]
    fn no_warning_when_prefix_matches() {
        let mut ids = vec![Some("foo-abc".to_owned())];
        let mut used: HashSet<String> = ["foo-abc".to_owned()].into_iter().collect();
        let mut rng = seeded_rng();
        let (_, warnings) = assign_ids(&mut ids, &mut used, "foo", &mut rng).unwrap();
        assert!(warnings.is_empty());
    }

    // @spec SYNC-ID-006
    #[test]
    fn aborts_on_duplicate_ids() {
        let mut ids = vec![
            Some("foo-abc".to_owned()),
            Some("foo-def".to_owned()),
            Some("foo-abc".to_owned()),
        ];
        let mut used: HashSet<String> = HashSet::new();
        let mut rng = seeded_rng();
        let err = assign_ids(&mut ids, &mut used, "foo", &mut rng).unwrap_err();
        assert_eq!(err, IdAssignmentError::DuplicateId("foo-abc".to_owned()));
    }

    // @spec SYNC-ID-006
    #[test]
    fn duplicate_detection_happens_before_any_assignment() {
        // The first entry needs an ID; entries 1 and 2 are duplicates.
        // The duplicate error must be returned without assigning anything.
        let mut ids = vec![None, Some("foo-aaa".to_owned()), Some("foo-aaa".to_owned())];
        let mut used: HashSet<String> = HashSet::new();
        let mut rng = seeded_rng();
        let result = assign_ids(&mut ids, &mut used, "foo", &mut rng);
        assert!(result.is_err());
        // The None slot must remain None — no partial assignment.
        assert!(
            ids[0].is_none(),
            "partial assignment must not occur on duplicate error"
        );
    }
}
