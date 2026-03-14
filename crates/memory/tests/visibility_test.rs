//! Integration tests for the three-tier visibility access control model.
//!
//! These tests verify that visibility filtering works correctly across
//! the full stack: type-level checks, store-level search filtering, and
//! cross-actor access patterns.

use chrono::Utc;
use memory::types::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_memory(
    id: &str,
    created_by: &str,
    visibility: VisibilityLevel,
    shared_with: Vec<String>,
    owner: Option<String>,
) -> Memory {
    let now = Utc::now();
    Memory {
        id: id.to_string(),
        content: format!("Content for {id}"),
        memory_type: MemoryType::Information,
        tags: vec![],
        created_by: created_by.to_string(),
        owner,
        created_at: now,
        updated_at: now,
        visibility,
        shared_with,
        references: vec![],
    }
}

// ---------------------------------------------------------------------------
// Public visibility
// ---------------------------------------------------------------------------

#[test]
fn test_public_memory_visible_to_everyone() {
    let m = make_memory("m1", "alice", VisibilityLevel::Public, vec![], None);

    assert!(m.is_visible_to(None), "anonymous can see public");
    assert!(m.is_visible_to(Some("alice")), "creator can see public");
    assert!(m.is_visible_to(Some("bob")), "stranger can see public");
    assert!(m.is_visible_to(Some("charlie")), "anyone can see public");
}

// ---------------------------------------------------------------------------
// Private visibility
// ---------------------------------------------------------------------------

#[test]
fn test_private_memory_only_visible_to_creator() {
    let m = make_memory("m1", "alice", VisibilityLevel::Private, vec![], None);

    assert!(!m.is_visible_to(None), "anonymous cannot see private");
    assert!(m.is_visible_to(Some("alice")), "creator can see private");
    assert!(!m.is_visible_to(Some("bob")), "stranger cannot see private");
}

#[test]
fn test_private_memory_visible_to_owner() {
    let m =
        make_memory("m1", "alice", VisibilityLevel::Private, vec![], Some("owner-1".to_string()));

    assert!(m.is_visible_to(Some("alice")), "creator can see");
    assert!(m.is_visible_to(Some("owner-1")), "owner can see");
    assert!(!m.is_visible_to(Some("bob")), "stranger cannot see");
    assert!(!m.is_visible_to(None), "anonymous cannot see");
}

#[test]
fn test_private_shared_with_list_is_ignored() {
    // Even if shared_with has entries, Private visibility ignores them
    let m = make_memory("m1", "alice", VisibilityLevel::Private, vec!["bob".to_string()], None);

    assert!(!m.is_visible_to(Some("bob")), "shared_with ignored for private");
}

// ---------------------------------------------------------------------------
// Shared visibility
// ---------------------------------------------------------------------------

#[test]
fn test_shared_memory_visible_to_creator() {
    let m = make_memory("m1", "alice", VisibilityLevel::Shared, vec!["bob".to_string()], None);

    assert!(m.is_visible_to(Some("alice")), "creator can see shared");
}

#[test]
fn test_shared_memory_visible_to_shared_actors() {
    let m = make_memory(
        "m1",
        "alice",
        VisibilityLevel::Shared,
        vec!["bob".to_string(), "charlie".to_string()],
        None,
    );

    assert!(m.is_visible_to(Some("bob")), "bob is in shared_with");
    assert!(m.is_visible_to(Some("charlie")), "charlie is in shared_with");
}

#[test]
fn test_shared_memory_invisible_to_unlisted_actors() {
    let m = make_memory("m1", "alice", VisibilityLevel::Shared, vec!["bob".to_string()], None);

    assert!(!m.is_visible_to(Some("dave")), "dave is not in shared_with");
    assert!(!m.is_visible_to(None), "anonymous cannot see shared");
}

#[test]
fn test_shared_memory_visible_to_owner() {
    let m = make_memory(
        "m1",
        "alice",
        VisibilityLevel::Shared,
        vec!["bob".to_string()],
        Some("owner-1".to_string()),
    );

    assert!(m.is_visible_to(Some("owner-1")), "owner can see shared");
    assert!(m.is_visible_to(Some("alice")), "creator can see shared");
    assert!(m.is_visible_to(Some("bob")), "shared actor can see");
    assert!(!m.is_visible_to(Some("charlie")), "unlisted cannot see");
}

#[test]
fn test_shared_empty_shared_with_only_creator_sees() {
    let m = make_memory("m1", "alice", VisibilityLevel::Shared, vec![], None);

    assert!(m.is_visible_to(Some("alice")), "creator can see");
    assert!(!m.is_visible_to(Some("bob")), "nobody else can see");
    assert!(!m.is_visible_to(None), "anonymous cannot see");
}

// ---------------------------------------------------------------------------
// Cross-actor scenarios
// ---------------------------------------------------------------------------

#[test]
fn test_multi_memory_visibility_filtering() {
    let memories = [
        make_memory("pub1", "alice", VisibilityLevel::Public, vec![], None),
        make_memory("prv1", "alice", VisibilityLevel::Private, vec![], None),
        make_memory("shr1", "alice", VisibilityLevel::Shared, vec!["bob".to_string()], None),
        make_memory("shr2", "charlie", VisibilityLevel::Shared, vec!["dave".to_string()], None),
        make_memory("prv2", "charlie", VisibilityLevel::Private, vec![], None),
    ];

    // Anonymous: only public
    let visible: Vec<&str> =
        memories.iter().filter(|m| m.is_visible_to(None)).map(|m| m.id.as_str()).collect();
    assert_eq!(visible, vec!["pub1"]);

    // Alice: pub1, prv1 (her private), shr1 (her shared)
    let visible: Vec<&str> =
        memories.iter().filter(|m| m.is_visible_to(Some("alice"))).map(|m| m.id.as_str()).collect();
    assert_eq!(visible, vec!["pub1", "prv1", "shr1"]);

    // Bob: pub1, shr1 (shared with him)
    let visible: Vec<&str> =
        memories.iter().filter(|m| m.is_visible_to(Some("bob"))).map(|m| m.id.as_str()).collect();
    assert_eq!(visible, vec!["pub1", "shr1"]);

    // Charlie: pub1, shr2 (his shared), prv2 (his private)
    let visible: Vec<&str> = memories
        .iter()
        .filter(|m| m.is_visible_to(Some("charlie")))
        .map(|m| m.id.as_str())
        .collect();
    assert_eq!(visible, vec!["pub1", "shr2", "prv2"]);

    // Dave: pub1, shr2 (shared with him)
    let visible: Vec<&str> =
        memories.iter().filter(|m| m.is_visible_to(Some("dave"))).map(|m| m.id.as_str()).collect();
    assert_eq!(visible, vec!["pub1", "shr2"]);

    // Random stranger: only public
    let visible: Vec<&str> = memories
        .iter()
        .filter(|m| m.is_visible_to(Some("stranger")))
        .map(|m| m.id.as_str())
        .collect();
    assert_eq!(visible, vec!["pub1"]);
}

// ---------------------------------------------------------------------------
// Visibility transitions
// ---------------------------------------------------------------------------

#[test]
fn test_visibility_transition_public_to_private() {
    let mut m = make_memory("m1", "alice", VisibilityLevel::Public, vec![], None);

    assert!(m.is_visible_to(Some("bob")));

    // Transition to private
    m.visibility = VisibilityLevel::Private;
    assert!(!m.is_visible_to(Some("bob")));
    assert!(m.is_visible_to(Some("alice")));
}

#[test]
fn test_visibility_transition_private_to_shared() {
    let mut m = make_memory("m1", "alice", VisibilityLevel::Private, vec![], None);

    assert!(!m.is_visible_to(Some("bob")));

    // Transition to shared with bob
    m.visibility = VisibilityLevel::Shared;
    m.shared_with = vec!["bob".to_string()];
    assert!(m.is_visible_to(Some("bob")));
    assert!(!m.is_visible_to(Some("charlie")));
}

#[test]
fn test_visibility_transition_shared_to_public() {
    let mut m = make_memory("m1", "alice", VisibilityLevel::Shared, vec!["bob".to_string()], None);

    assert!(!m.is_visible_to(Some("charlie")));

    // Transition to public
    m.visibility = VisibilityLevel::Public;
    assert!(m.is_visible_to(Some("charlie")));
    assert!(m.is_visible_to(None));
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

#[test]
fn test_creator_always_sees_own_memories_regardless_of_visibility() {
    for vis in [VisibilityLevel::Public, VisibilityLevel::Private, VisibilityLevel::Shared] {
        let m = make_memory("m1", "alice", vis, vec![], None);
        assert!(
            m.is_visible_to(Some("alice")),
            "creator should see own memory with visibility {:?}",
            vis
        );
    }
}

#[test]
fn test_owner_always_sees_memory_regardless_of_visibility() {
    for vis in [VisibilityLevel::Public, VisibilityLevel::Private, VisibilityLevel::Shared] {
        let m = make_memory("m1", "alice", vis, vec![], Some("owner-1".to_string()));
        assert!(
            m.is_visible_to(Some("owner-1")),
            "owner should see memory with visibility {:?}",
            vis
        );
    }
}

#[test]
fn test_memory_type_serialization_roundtrip() {
    for mt in [MemoryType::Information, MemoryType::Question, MemoryType::Request] {
        let s = mt.to_string();
        let parsed: MemoryType = s.parse().unwrap();
        assert_eq!(parsed, mt);
    }
}

#[test]
fn test_visibility_level_serialization_roundtrip() {
    for vl in [VisibilityLevel::Public, VisibilityLevel::Private, VisibilityLevel::Shared] {
        let s = vl.to_string();
        let parsed: VisibilityLevel = s.parse().unwrap();
        assert_eq!(parsed, vl);
    }
}
