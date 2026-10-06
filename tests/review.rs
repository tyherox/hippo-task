//! Human review must not silently replace concurrent agent work or export
//! different rows from the ones that were reviewed.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod common;

use common::TempDir;
use hippo_task::error::Error;
use hippo_task::model::{Priority, State};
use hippo_task::ops::{self, Changes, Ctx, Destination, ExportSelection, NewTask};
use hippo_task::store::Store;

fn ctx() -> Ctx {
    Ctx::new(None, None, 1).unwrap()
}

fn add(store: &Store, title: &str) -> hippo_task::model::Task {
    ops::add(
        store,
        &ctx(),
        NewTask {
            title: title.into(),
            priority: Priority::None,
            body: None,
            assignee: None,
            labels: vec![],
            fields: vec![],
        },
    )
    .unwrap()
}

#[test]
fn stale_metadata_save_writes_nothing_and_keeps_the_newer_title() {
    let dir = TempDir::new("review-stale");
    let store = Store::new(dir.path());
    let t = add(&store, "Original");
    ops::update(
        &store,
        &ctx(),
        "1",
        Changes {
            title: Some("Agent edit".into()),
            ..Changes::default()
        },
    )
    .unwrap();
    let before = store.read().unwrap().events.len();
    let result = ops::update_checked(
        &store,
        &ctx(),
        "1",
        Changes {
            title: Some("Human edit".into()),
            body: Some("Human description".into()),
            ..Changes::default()
        },
        t.seq,
    );
    assert!(matches!(result, Err(Error::Stale(_))));
    assert_eq!(store.read().unwrap().events.len(), before);
    let latest = ops::show(&store, "1").unwrap().task;
    assert_eq!(latest.title, "Agent edit");
    assert_eq!(latest.body, None);
}

#[test]
fn checked_body_only_edit_merges_paragraphs_but_conflicts_write_nothing() {
    let dir = TempDir::new("review-body");
    let store = Store::new(dir.path());
    add(&store, "Description");
    let base = ops::update(
        &store,
        &ctx(),
        "1",
        Changes {
            body: Some("One\n\nTwo".into()),
            ..Changes::default()
        },
    )
    .unwrap();
    ops::update(
        &store,
        &ctx(),
        "1",
        Changes {
            body: Some("Agent one\n\nTwo".into()),
            ..Changes::default()
        },
    )
    .unwrap();
    let merged = ops::update_checked(
        &store,
        &ctx(),
        "1",
        Changes {
            body: Some("One\n\nHuman two".into()),
            ..Changes::default()
        },
        base.seq,
    )
    .unwrap();
    assert_eq!(merged.body.as_deref(), Some("Agent one\n\nHuman two"));
    let before = store.read().unwrap().events.len();
    let conflict = ops::update_checked(
        &store,
        &ctx(),
        "1",
        Changes {
            body: Some("Different one\n\nTwo".into()),
            ..Changes::default()
        },
        base.seq,
    );
    assert!(matches!(conflict, Err(Error::Stale(_))));
    assert_eq!(store.read().unwrap().events.len(), before);
}

#[test]
fn checked_state_changes_obey_claims_and_doing_does_not_claim() {
    let dir = TempDir::new("review-state");
    let store = Store::new(dir.path());
    let t = add(&store, "Move me");
    let doing = ops::update_checked(
        &store,
        &ctx(),
        "1",
        Changes {
            state: Some(State::Doing),
            ..Changes::default()
        },
        t.seq,
    )
    .unwrap();
    assert_eq!(doing.state, State::Doing);
    assert!(doing.lease.is_none());
    let agent = Ctx::new(Some("agent:worker".into()), Some("worker-1".into()), 2).unwrap();
    let held = ops::start(&store, &agent, "1").unwrap();
    let before = store.read().unwrap().events.len();
    assert!(matches!(
        ops::update_checked(
            &store,
            &ctx(),
            "1",
            Changes {
                state: Some(State::Done),
                ..Changes::default()
            },
            held.seq
        ),
        Err(Error::Conflict(_))
    ));
    assert_eq!(store.read().unwrap().events.len(), before);
}

#[test]
fn export_review_selects_exact_ids_and_records_only_after_save() {
    let dir = TempDir::new("review-export");
    let store = Store::new(dir.path());
    add(&store, "Leave behind");
    let selected = add(&store, "Send this");
    let selection = ExportSelection {
        ids: Some(vec![selected.id.clone()]),
        ..ExportSelection::default()
    };
    let before = store.read().unwrap().events.len();
    let preview = ops::export(&store, &ctx(), Destination::Notion, &selection, None).unwrap();
    assert_eq!(preview.tasks.len(), 1);
    assert_eq!(preview.tasks[0].id, selected.id);
    assert!(!preview.csv.contains("Leave behind"));
    assert_eq!(store.read().unwrap().events.len(), before);
    let path = dir.path().join("selected.csv");
    let saved = ops::export_reviewed(
        &store,
        &ctx(),
        Destination::Notion,
        &selection,
        &path,
        &preview.review,
    )
    .unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), preview.csv);
    assert_eq!(saved.tasks.len(), 1);
    assert!(ops::show(&store, "1").unwrap().task.exported.is_empty());
    assert!(ops::show(&store, "2")
        .unwrap()
        .task
        .exported
        .contains_key("notion"));
    assert_eq!(store.read().unwrap().events.len(), before + 1);
}

#[test]
fn reviewed_export_catches_blocker_changes_and_export_bookkeeping() {
    let dir = TempDir::new("review-blocker");
    let store = Store::new(dir.path());
    let blocker = add(&store, "Dependency");
    let selected = add(&store, "Ship");
    ops::update(
        &store,
        &ctx(),
        "2",
        Changes {
            block: vec![blocker.id],
            ..Changes::default()
        },
    )
    .unwrap();
    let selection = ExportSelection {
        ids: Some(vec![selected.id]),
        ..ExportSelection::default()
    };
    let preview = ops::export(&store, &ctx(), Destination::Notion, &selection, None).unwrap();
    let seq = ops::show(&store, "2").unwrap().task.seq;
    ops::done(&store, &ctx(), "1", false).unwrap();
    assert_eq!(ops::show(&store, "2").unwrap().task.seq, seq);
    let path = dir.path().join("stale.csv");
    let before = store.read().unwrap().events.len();
    assert!(matches!(
        ops::export_reviewed(
            &store,
            &ctx(),
            Destination::Notion,
            &selection,
            &path,
            &preview.review
        ),
        Err(Error::Stale(_))
    ));
    assert!(!path.exists());
    assert_eq!(store.read().unwrap().events.len(), before);
    let fresh = ops::export(&store, &ctx(), Destination::Notion, &selection, None).unwrap();
    ops::export(
        &store,
        &ctx(),
        Destination::Notion,
        &selection,
        Some(&dir.path().join("other.csv")),
    )
    .unwrap();
    assert!(matches!(
        ops::export_reviewed(
            &store,
            &ctx(),
            Destination::Notion,
            &selection,
            &path,
            &fresh.review
        ),
        Err(Error::Stale(_))
    ));
    assert!(!path.exists());
}

#[test]
fn export_selection_rejects_missing_ids_and_empty_means_none() {
    let dir = TempDir::new("review-ids");
    let store = Store::new(dir.path());
    add(&store, "Keep");
    let empty = ExportSelection {
        ids: Some(vec![]),
        ..ExportSelection::default()
    };
    assert!(
        ops::export(&store, &ctx(), Destination::Notion, &empty, None)
            .unwrap()
            .tasks
            .is_empty()
    );
    let missing = ExportSelection {
        ids: Some(vec!["999".into()]),
        ..ExportSelection::default()
    };
    assert!(matches!(
        ops::export(&store, &ctx(), Destination::Notion, &missing, None),
        Err(Error::NotFound(_))
    ));
}

#[test]
fn export_review_checks_column_names_but_notes_do_not_change_rows() {
    let dir = TempDir::new("review-config");
    let store = Store::new(dir.path());
    add(&store, "Review columns");
    let config = store.folder().join("config.toml");
    std::fs::write(&config, "[fields.area]\ndisplay_name = 'Area'\n").unwrap();
    let selection = ExportSelection::default();
    let preview = ops::export(&store, &ctx(), Destination::Notion, &selection, None).unwrap();
    std::fs::write(&config, "[fields.area]\ndisplay_name = 'Work area'\n").unwrap();
    let path = dir.path().join("columns.csv");
    assert!(matches!(
        ops::export_reviewed(
            &store,
            &ctx(),
            Destination::Notion,
            &selection,
            &path,
            &preview.review
        ),
        Err(Error::Stale(_))
    ));
    assert!(!path.exists());
    let fresh = ops::export(&store, &ctx(), Destination::Notion, &selection, None).unwrap();
    ops::note(&store, &ctx(), "1", "Still reviewing").unwrap();
    let saved = ops::export_reviewed(
        &store,
        &ctx(),
        Destination::Notion,
        &selection,
        &path,
        &fresh.review,
    )
    .unwrap();
    assert_eq!(saved.csv, fresh.csv);
    assert!(saved.csv.contains("Work area"));
}
