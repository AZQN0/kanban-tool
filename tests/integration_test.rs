use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn kanban_bin() -> &'static str {
    env!("CARGO_BIN_EXE_kanban")
}

struct TestProject {
    path: PathBuf,
}

impl TestProject {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "kanban_integration_{}_{}_{}",
            std::process::id(),
            name,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path_arg(&self) -> String {
        self.path.to_string_lossy().into_owned()
    }
}

impl Drop for TestProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn run(args: &[&str], cwd: Option<&Path>) -> Output {
    let mut command = Command::new(kanban_bin());
    command.args(args);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    command.output().unwrap()
}

fn assert_success(output: Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        output.status.success(),
        "expected command to succeed\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
    stdout
}

fn assert_failure(output: Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        !output.status.success(),
        "expected command to fail\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
    format!("{}\n{}", stdout, stderr)
}

fn create_card(project: &TestProject, title: &str, description: &str) -> String {
    let project_path = project.path_arg();
    let stdout = assert_success(run(
        &[
            "create",
            "--project",
            &project_path,
            "--title",
            title,
            "--description",
            description,
            "--priority",
            "high",
            "--column",
            "backlog",
            "--label",
            "backend",
        ],
        None,
    ));

    stdout
        .lines()
        .find_map(|line| line.strip_prefix("Created card: "))
        .expect("created card output should include card id")
        .to_string()
}

#[test]
fn cli_lifecycle_init_create_list_get_move_update_search_delete() {
    let project = TestProject::new("lifecycle");
    let project_path = project.path_arg();

    let init = assert_success(run(&["init", &project_path], None));
    assert!(init.contains("Initialized kanban board at:"));
    assert!(project.path.join(".kanban/kanban.db").exists());

    let card_id = create_card(
        &project,
        "Workflow task",
        "Exercise create list get move update search delete",
    );

    let list = assert_success(run(&["list", "--project", &project_path], None));
    assert!(list.contains("Workflow task"), "{list}");
    assert!(list.contains("backlog"), "{list}");

    let get = assert_success(run(&["get", &card_id], Some(&project.path)));
    assert!(get.contains("Title:      Workflow task"), "{get}");
    assert!(get.contains("Labels:     backend"), "{get}");

    let moved = assert_success(run(&["move", &card_id, "in_progress"], Some(&project.path)));
    assert!(moved.contains("Moved card"), "{moved}");
    assert!(moved.contains("in_progress"), "{moved}");

    let updated = assert_success(run(
        &[
            "update",
            &card_id,
            "--title",
            "Updated workflow task",
            "--priority",
            "urgent",
        ],
        Some(&project.path),
    ));
    assert!(updated.contains("Updated card:"), "{updated}");
    assert!(updated.contains("Updated workflow task"), "{updated}");

    let after_update = assert_success(run(&["get", &card_id], Some(&project.path)));
    assert!(after_update.contains("Title:      Updated workflow task"));
    assert!(after_update.contains("Priority:   urgent"));
    assert!(
        after_update.contains("Labels:     backend"),
        "updating without labels should preserve existing labels\n{after_update}"
    );

    let search = assert_success(run(
        &["search", "workflow", "--project", &project_path],
        None,
    ));
    assert!(search.contains("Updated workflow task"), "{search}");
    assert!(search.contains("in_progress"), "{search}");

    let deleted = assert_success(run(&["delete", &card_id], Some(&project.path)));
    assert!(deleted.contains("Deleted card:"), "{deleted}");

    let missing = assert_failure(run(&["get", &card_id], Some(&project.path)));
    assert!(missing.contains("Card not found"), "{missing}");
}

#[test]
fn cli_project_flag_keeps_boards_isolated() {
    let first = TestProject::new("first");
    let second = TestProject::new("second");
    let first_path = first.path_arg();
    let second_path = second.path_arg();

    assert_success(run(&["init", &first_path], None));
    assert_success(run(&["init", &second_path], None));

    create_card(&first, "First board only", "first");
    create_card(&second, "Second board only", "second");

    let first_list = assert_success(run(&["list", "--project", &first_path], None));
    assert!(first_list.contains("First board only"), "{first_list}");
    assert!(!first_list.contains("Second board only"), "{first_list}");

    let second_search = assert_success(run(&["search", "Second", "--project", &second_path], None));
    assert!(
        second_search.contains("Second board only"),
        "{second_search}"
    );
    assert!(
        !second_search.contains("First board only"),
        "{second_search}"
    );
}

#[test]
fn cli_accepts_documented_web_ui_command_spelling() {
    let help = assert_success(run(&["web-ui", "--help"], None));

    assert!(help.contains("Start the WebUI"), "{help}");
    assert!(help.contains("--allow-remote"), "{help}");

    let misspelled = assert_failure(run(&["webui", "--help"], None));
    assert!(
        misspelled.contains("unrecognized subcommand"),
        "{misspelled}"
    );
    assert!(misspelled.contains("web-ui"), "{misspelled}");
}
