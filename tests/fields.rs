//! Fields (ADR-006): declared attributes with one value each — `project`,
//! `team`, or whatever a team declares in its store's `config.toml` — and
//! filtering tasks by field or by label.
// Tests may panic — that's how a test fails. (Product code may not: see Cargo.toml [lints].)
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{ok, tasks, TempDir, HUMAN};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

const CONFIG: &str = r#"
[fields.project]
values = ["dashboard", "billing"]

[fields.team]
values = ["engineering-whiteboards", "design"]

[fields.customer]   # no list: any text
display_name = "Client"
"#;

/// A project whose store declares the fields above.
fn with_fields(tag: &str) -> TempDir {
    let dir = TempDir::new(tag);
    write_config(dir.path(), CONFIG);
    dir
}

fn write_config(project: &Path, text: &str) {
    let store = project.join(".hippotask");
    fs::create_dir_all(&store).unwrap();
    fs::write(store.join("config.toml"), text).unwrap();
}

fn fields_of(dir: &Path, id: &str) -> Value {
    ok(dir, HUMAN, &["show", id, "--json"]).json()["fields"].clone()
}

/// The task numbers `list` returns for these extra arguments.
fn listed(dir: &Path, args: &[&str]) -> Vec<u64> {
    let mut all = vec!["list", "--json"];
    all.extend_from_slice(args);
    ok(dir, HUMAN, &all)
        .json()
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["num"].as_u64().unwrap())
        .collect()
}

// ---- setting fields ----

#[test]
fn declared_fields_are_set_when_adding_and_shown_in_json() {
    let dir = with_fields("fields-add");
    let t = ok(
        dir.path(),
        HUMAN,
        &[
            "add",
            "Design the toolbar",
            "--field",
            "project=dashboard",
            "--field",
            "team=design",
            "--json",
        ],
    )
    .json();
    assert_eq!(
        t["fields"],
        json!({"project": "dashboard", "team": "design"})
    );
}

#[test]
fn a_task_without_fields_has_an_empty_fields_object() {
    let dir = TempDir::new("fields-none");
    let t = ok(dir.path(), HUMAN, &["add", "x", "--json"]).json();
    assert_eq!(t["fields"], json!({}));
}

#[test]
fn a_value_off_the_list_is_refused_with_the_closest_match() {
    let dir = with_fields("fields-bad-value");
    let run = tasks(
        dir.path(),
        HUMAN,
        &["add", "x", "--field", "project=dashbaord"],
    );
    assert_eq!(run.code, 2, "{run:#?}");
    assert!(run.stderr.contains("did you mean"), "{}", run.stderr);
    assert!(run.stderr.contains("dashboard"), "{}", run.stderr);
    assert_eq!(
        listed(dir.path(), &[]),
        Vec::<u64>::new(),
        "nothing was added"
    );
}

#[test]
fn an_undeclared_field_is_refused_with_the_closest_match() {
    let dir = with_fields("fields-bad-name");
    let run = tasks(
        dir.path(),
        HUMAN,
        &["add", "x", "--field", "projct=dashboard"],
    );
    assert_eq!(run.code, 2, "{run:#?}");
    assert!(run.stderr.contains("did you mean"), "{}", run.stderr);
    assert!(run.stderr.contains("project"), "{}", run.stderr);
}

#[test]
fn without_a_config_a_field_is_refused_and_the_error_says_where_to_declare_it() {
    let dir = TempDir::new("fields-no-config");
    let run = tasks(
        dir.path(),
        HUMAN,
        &["add", "x", "--field", "project=dashboard"],
    );
    assert_eq!(run.code, 2, "{run:#?}");
    assert!(run.stderr.contains("config.toml"), "{}", run.stderr);
}

#[test]
fn a_field_holds_one_value_setting_it_again_replaces_it_and_clearing_removes_it() {
    let dir = with_fields("fields-replace");
    let d = dir.path();
    ok(
        d,
        HUMAN,
        &[
            "add",
            "x",
            "--field",
            "project=dashboard",
            "--field",
            "team=design",
        ],
    );
    ok(d, HUMAN, &["update", "1", "--field", "project=billing"]);
    assert_eq!(
        fields_of(d, "1"),
        json!({"project": "billing", "team": "design"})
    );
    ok(d, HUMAN, &["update", "1", "--clear-field", "team"]);
    assert_eq!(fields_of(d, "1"), json!({"project": "billing"}));
}

#[test]
fn one_command_can_set_a_field_only_once() {
    let dir = with_fields("fields-twice");
    let run = tasks(
        dir.path(),
        HUMAN,
        &[
            "add",
            "x",
            "--field",
            "project=dashboard",
            "--field",
            "project=billing",
        ],
    );
    assert_eq!(run.code, 2, "{run:#?}");
    assert!(run.stderr.contains("project"), "{}", run.stderr);
}

#[test]
fn a_field_without_a_list_takes_any_text() {
    let dir = with_fields("fields-free-text");
    let t = ok(
        dir.path(),
        HUMAN,
        &["add", "x", "--field", "customer=Acme, Inc.", "--json"],
    )
    .json();
    assert_eq!(t["fields"]["customer"], "Acme, Inc.");
}

#[test]
fn a_field_argument_is_name_equals_value() {
    let dir = with_fields("fields-syntax");
    for bad in ["project", "=dashboard", "project="] {
        let run = tasks(dir.path(), HUMAN, &["add", "x", "--field", bad]);
        assert_eq!(run.code, 2, "{bad}: {run:#?}");
    }
    let run = tasks(dir.path(), HUMAN, &["add", "x", "--field", "project="]);
    assert!(
        run.stderr.contains("--clear-field"),
        "an empty value points to --clear-field: {}",
        run.stderr
    );
}

#[test]
fn labels_stay_free_form_when_fields_are_declared() {
    let dir = with_fields("fields-labels-free");
    let t = ok(
        dir.path(),
        HUMAN,
        &[
            "add",
            "x",
            "--label",
            "anything-goes",
            "--label",
            "project:whatever",
            "--json",
        ],
    )
    .json();
    assert_eq!(t["labels"], json!(["anything-goes", "project:whatever"]));
}

// ---- finding tasks by field and by label ----

#[test]
fn list_filters_by_field_and_by_label() {
    let dir = with_fields("fields-filter");
    let d = dir.path();
    ok(
        d,
        HUMAN,
        &["add", "a", "--field", "project=dashboard", "--label", "bug"],
    );
    ok(d, HUMAN, &["add", "b", "--field", "project=dashboard"]);
    ok(
        d,
        HUMAN,
        &["add", "c", "--field", "project=billing", "--label", "bug"],
    );
    ok(d, HUMAN, &["add", "d"]);
    assert_eq!(listed(d, &["--field", "project=dashboard"]), [1, 2]);
    assert_eq!(listed(d, &["--label", "bug"]), [1, 3]);
    assert_eq!(
        listed(d, &["--field", "project=dashboard", "--label", "bug"]),
        [1],
        "every filter must match"
    );
}

#[test]
fn a_filter_on_a_value_off_the_list_warns_but_still_runs() {
    let dir = with_fields("fields-filter-typo");
    let d = dir.path();
    ok(d, HUMAN, &["add", "a", "--field", "project=dashboard"]);
    let run = ok(d, HUMAN, &["list", "--field", "project=dashbaord"]);
    assert!(run.stdout.trim().is_empty(), "{run:#?}");
    assert!(
        run.stderr.contains("dashboard"),
        "suggests a value: {}",
        run.stderr
    );
    // In --json mode the warning is a JSON line on stderr.
    let run = ok(
        d,
        HUMAN,
        &["list", "--field", "project=dashbaord", "--json"],
    );
    let warning: Value = serde_json::from_str(run.stderr.lines().next().unwrap()).unwrap();
    assert!(
        warning["warning"].as_str().unwrap().contains("dashboard"),
        "{warning}"
    );
}

#[test]
fn a_filter_on_an_undeclared_field_is_refused() {
    let dir = with_fields("fields-filter-name");
    let run = tasks(dir.path(), HUMAN, &["list", "--field", "projct=dashboard"]);
    assert_eq!(run.code, 2, "{run:#?}");
    assert!(run.stderr.contains("project"), "{}", run.stderr);
}

// ---- the `fields` report ----

#[test]
fn fields_lists_what_is_declared_and_how_much_each_value_is_used() {
    let dir = with_fields("fields-report");
    let d = dir.path();
    ok(d, HUMAN, &["add", "a", "--field", "project=dashboard"]);
    ok(d, HUMAN, &["add", "b", "--field", "project=dashboard"]);
    ok(d, HUMAN, &["add", "c", "--field", "customer=Acme"]);
    ok(d, HUMAN, &["done", "2"]);

    let report = ok(d, HUMAN, &["fields", "--json"]).json();
    assert!(
        report["config"].as_str().unwrap().ends_with("config.toml"),
        "{report}"
    );
    let field = |name: &str| -> Value {
        report["fields"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["field"] == name)
            .unwrap_or_else(|| panic!("no {name} in {report}"))
            .clone()
    };
    let count = |f: &Value, value: &str| -> (u64, u64) {
        let c = f["counts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["value"] == value)
            .unwrap_or_else(|| panic!("no count for {value} in {f}"));
        (c["open"].as_u64().unwrap(), c["total"].as_u64().unwrap())
    };

    let project = field("project");
    assert_eq!(project["display_name"], "Project");
    assert_eq!(project["values"], json!(["dashboard", "billing"]));
    assert_eq!(count(&project, "dashboard"), (1, 2));
    assert_eq!(
        count(&project, "billing"),
        (0, 0),
        "every allowed value is listed"
    );
    let customer = field("customer");
    assert_eq!(customer["display_name"], "Client");
    assert_eq!(customer["values"], Value::Null, "any text");
    assert_eq!(count(&customer, "Acme"), (1, 1));
    assert_eq!(report["strays"], json!([]));

    let text = ok(d, HUMAN, &["fields"]).stdout;
    for needle in [
        "project",
        "dashboard",
        "billing",
        "team",
        "design",
        "customer",
        "Client",
    ] {
        assert!(text.contains(needle), "{needle} missing from:\n{text}");
    }
}

#[test]
fn values_the_config_no_longer_allows_are_kept_and_reported() {
    let dir = with_fields("fields-strays");
    let d = dir.path();
    let id = ok(
        d,
        HUMAN,
        &["add", "a", "--field", "project=billing", "--json"],
    )
    .json()["id"]
        .as_str()
        .unwrap()
        .to_string();
    // The team retires `billing`.
    write_config(
        d,
        &CONFIG.replace(r#""dashboard", "billing""#, r#""dashboard""#),
    );
    assert_eq!(
        fields_of(d, "1")["project"],
        "billing",
        "the task keeps its value"
    );

    let report = ok(d, HUMAN, &["fields", "--json"]).json();
    assert_eq!(
        report["strays"],
        json!([{"num": 1, "id": id, "field": "project", "value": "billing"}])
    );
    assert!(ok(d, HUMAN, &["fields"]).stdout.contains("billing"));

    // It can't be set again, but it can always be cleared.
    let run = tasks(d, HUMAN, &["update", "1", "--field", "project=billing"]);
    assert_eq!(run.code, 2, "{run:#?}");
    ok(d, HUMAN, &["update", "1", "--clear-field", "project"]);
    assert_eq!(fields_of(d, "1"), json!({}));
}

#[test]
fn a_broken_config_is_a_usage_error_that_names_the_file() {
    let dir = TempDir::new("fields-broken-config");
    let d = dir.path();
    for (text, mentions) in [
        ("[fields.project\n", "config.toml"),               // not TOML
        ("[fields.project]\nvaules = [\"x\"]\n", "vaules"), // a misspelled setting
        ("[fields.\"Project Name\"]\n", "Project Name"),    // not a usable field name
        ("[fields.project]\nvalues = []\n", "project"),     // a list with nothing on it
    ] {
        write_config(d, text);
        let run = tasks(d, HUMAN, &["fields"]);
        assert_eq!(run.code, 2, "{text:?}: {run:#?}");
        assert!(
            run.stderr.contains("config.toml"),
            "{text:?}: {}",
            run.stderr
        );
        assert!(run.stderr.contains(mentions), "{text:?}: {}", run.stderr);
    }
}

#[test]
fn a_display_name_must_not_repeat_a_column_heading() {
    // Duplicate CSV headers break Notion's Import and Merge with CSV.
    let dir = TempDir::new("fields-display-clash");
    let d = dir.path();
    for (text, mentions) in [
        ("[fields.status]\n", "Status"), // the default name repeats a built-in column
        ("[fields.labels]\ndisplay_name = \"tags\"\n", "tags"), // ignoring case
        (
            "[fields.a]\ndisplay_name = \"Client\"\n[fields.b]\ndisplay_name = \"Client\"\n",
            "Client",
        ),
    ] {
        write_config(d, text);
        let run = tasks(d, HUMAN, &["fields"]);
        assert_eq!(run.code, 2, "{text:?}: {run:#?}");
        assert!(run.stderr.contains(mentions), "{text:?}: {}", run.stderr);
        assert!(
            run.stderr.contains("display_name"),
            "{text:?}: {}",
            run.stderr
        );
    }
    write_config(d, "[fields.status]\ndisplay_name = \"Stage\"\n");
    ok(d, HUMAN, &["fields"]);
}

#[test]
fn a_setting_this_version_does_not_know_is_a_warning_not_an_error() {
    // A newer hippo-task may add sections; everyone sharing the store keeps working.
    let dir = TempDir::new("fields-unknown-setting");
    let d = dir.path();
    write_config(
        d,
        "[sync]\nurl = \"https://example.com\"\n\n[fields.project]\nvalues = [\"a\"]\n",
    );
    let run = ok(d, HUMAN, &["add", "x", "--field", "project=a"]);
    assert!(run.stderr.contains("sync"), "{}", run.stderr);
    let run = ok(d, HUMAN, &["list", "--json", "--field", "project=a"]);
    let warning: Value = serde_json::from_str(run.stderr.lines().next().unwrap_or("{}")).unwrap();
    assert!(
        warning["warning"].as_str().unwrap_or("").contains("sync"),
        "in --json mode it's a JSON line: {}",
        run.stderr
    );
    let report = ok(d, HUMAN, &["fields"]);
    assert_eq!(
        report.stderr.matches("sync").count(),
        1,
        "said once per command: {}",
        report.stderr
    );

    write_config(d, "[feilds.project]\nvalues = [\"a\"]\n");
    let run = ok(d, HUMAN, &["fields"]);
    assert!(
        run.stderr.contains("did you mean `fields`"),
        "{}",
        run.stderr
    );
}

#[test]
fn a_filter_names_a_field_once() {
    let dir = with_fields("fields-filter-twice");
    let run = tasks(
        dir.path(),
        HUMAN,
        &[
            "list",
            "--field",
            "project=dashboard",
            "--field",
            "project=billing",
        ],
    );
    assert_eq!(run.code, 2, "{run:#?}");
    assert!(run.stderr.contains("project"), "{}", run.stderr);
}

#[test]
fn without_a_config_fields_says_how_to_declare_some() {
    let dir = TempDir::new("fields-template");
    let run = ok(dir.path(), HUMAN, &["fields"]);
    assert!(
        run.stdout.contains("[fields."),
        "shows an example: {}",
        run.stdout
    );
    let report = ok(dir.path(), HUMAN, &["fields", "--json"]).json();
    assert_eq!(report["fields"], json!([]));
}

// ---- history, the guide, and stores elsewhere ----

#[test]
fn history_shows_field_changes() {
    let dir = with_fields("fields-history");
    let d = dir.path();
    ok(d, HUMAN, &["add", "a", "--field", "project=dashboard"]);
    let shown = ok(d, HUMAN, &["show", "1"]).stdout;
    assert!(shown.contains("project=dashboard"), "{shown}");
    ok(d, HUMAN, &["update", "1", "--clear-field", "project"]);
    let shown = ok(d, HUMAN, &["show", "1"]).stdout;
    assert!(shown.contains("set-field"), "{shown}");
    assert!(shown.contains("project → dashboard"), "{shown}");
    assert!(shown.contains("project → (cleared)"), "{shown}");
}

#[test]
fn the_guide_tells_agents_to_check_the_fields_before_setting_them() {
    let dir = TempDir::new("fields-guide");
    let guide = ok(dir.path(), HUMAN, &["guide"]).stdout;
    assert!(guide.contains("hippo-task fields"), "{guide}");
    assert!(guide.contains("--field"), "{guide}");
}

#[test]
fn a_store_in_another_folder_keeps_its_config_there() {
    let project = TempDir::new("fields-pointer-project");
    let elsewhere = TempDir::new("fields-pointer-store");
    let folder = elsewhere.path().to_str().unwrap();
    ok(project.path(), HUMAN, &["init", "--folder", folder]);
    fs::write(elsewhere.path().join("config.toml"), CONFIG).unwrap();
    let t = ok(
        project.path(),
        HUMAN,
        &["add", "x", "--field", "project=billing", "--json"],
    )
    .json();
    assert_eq!(t["fields"]["project"], "billing");
}
