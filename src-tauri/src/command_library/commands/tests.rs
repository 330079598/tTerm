use super::*;
use crate::command_library::CommandVariable;
use crate::db::Database;

fn create_input() -> SaveCommandInput {
    SaveCommandInput {
        id: None,
        name: "  Docker logs  ".to_string(),
        command_text: "docker logs -f api".to_string(),
        description: "  Follow logs  ".to_string(),
        tags: vec![
            " Docker ".to_string(),
            "docker".to_string(),
            "logs".to_string(),
        ],
        scope_type: "global".to_string(),
        scope_id: None,
        is_favorite: true,
        confirm_before_run: None,
        variables: None,
    }
}

fn variable(name: &str, value_type: &str) -> CommandVariable {
    CommandVariable {
        name: name.to_string(),
        label: String::new(),
        value_type: value_type.to_string(),
        default_value: None,
        options_json: None,
        is_required: true,
        position: 99,
    }
}

#[test]
fn creates_and_normalizes_user_input() {
    let database = Database::open_in_memory().expect("open database");
    let repository = CommandRepository::new(&database);

    let saved = save(&repository, create_input(), 100).expect("save command");

    assert_eq!(saved.name, "Docker logs");
    assert_eq!(saved.description, "Follow logs");
    assert_eq!(saved.tags, ["Docker", "logs"]);
    assert_eq!(saved.created_at, 100);
    assert_eq!(saved.updated_at, 100);
}

#[test]
fn updates_preserve_server_owned_and_future_fields() {
    let database = Database::open_in_memory().expect("open database");
    let repository = CommandRepository::new(&database);
    let mut original = save(&repository, create_input(), 100).expect("create command");
    original.use_count = 7;
    original.last_used_at = Some(150);
    original.variables.push(CommandVariable {
        name: "container".to_string(),
        label: "Container".to_string(),
        value_type: "text".to_string(),
        default_value: None,
        options_json: None,
        is_required: true,
        position: 0,
    });
    repository.save(&original).expect("seed future fields");

    let mut input = create_input();
    input.id = Some(original.id.clone());
    input.name = "Updated".to_string();
    let updated = save(&repository, input, 200).expect("update command");

    assert_eq!(updated.id, original.id);
    assert_eq!(updated.created_at, 100);
    assert_eq!(updated.updated_at, 200);
    assert_eq!(updated.use_count, 7);
    assert_eq!(updated.last_used_at, Some(150));
    assert_eq!(updated.variables, original.variables);
}

#[test]
fn saves_variables_and_confirmation_from_the_input() {
    let database = Database::open_in_memory().expect("open database");
    let repository = CommandRepository::new(&database);
    let mut input = create_input();
    input.command_text = "ssh {{user}}@{{host}} -p {{port}}".to_string();
    input.confirm_before_run = Some(true);
    input.variables = Some(vec![
        CommandVariable {
            label: "  Login  ".to_string(),
            default_value: Some("root".to_string()),
            // Options only belong to choices.
            options_json: Some("[\"a\"]".to_string()),
            ..variable(" user ", "text")
        },
        CommandVariable {
            default_value: Some("c".to_string()),
            options_json: Some("[\" a \", \"b\", \"a\", \"\"]".to_string()),
            ..variable("host", "choice")
        },
        CommandVariable {
            default_value: Some("hunter2".to_string()),
            ..variable("port", "secret")
        },
    ]);

    let saved = save(&repository, input, 100).expect("save command");
    assert!(saved.confirm_before_run);
    assert_eq!(
        saved.variables,
        vec![
            CommandVariable {
                name: "user".to_string(),
                label: "Login".to_string(),
                default_value: Some("root".to_string()),
                position: 0,
                ..variable("user", "text")
            },
            CommandVariable {
                label: "host".to_string(),
                // A default outside the options is dropped.
                default_value: None,
                options_json: Some("[\"a\",\"b\"]".to_string()),
                position: 1,
                ..variable("host", "choice")
            },
            CommandVariable {
                label: "port".to_string(),
                default_value: None,
                position: 2,
                ..variable("port", "secret")
            },
        ]
    );
    assert_eq!(
        repository.get(&saved.id).expect("get command"),
        Some(saved.clone())
    );

    // An explicit empty list removes them; `false` clears the confirmation.
    let mut update = create_input();
    update.id = Some(saved.id);
    update.confirm_before_run = Some(false);
    update.variables = Some(Vec::new());
    let updated = save(&repository, update, 200).expect("update command");
    assert!(!updated.confirm_before_run);
    assert!(updated.variables.is_empty());
}

#[test]
fn rejects_malformed_variables() {
    let database = Database::open_in_memory().expect("open database");
    let repository = CommandRepository::new(&database);
    let cases: [(Vec<CommandVariable>, &str); 5] = [
        (vec![variable("1st", "text")], "must start with a letter"),
        (vec![variable("a b", "text")], "must start with a letter"),
        (
            vec![variable("host", "text"), variable("host", "number")],
            "defined twice",
        ),
        (vec![variable("env", "choice")], "invalid options"),
        (
            vec![CommandVariable {
                default_value: Some("many".to_string()),
                ..variable("count", "number")
            }],
            "not a number",
        ),
    ];

    for (variables, expected) in cases {
        let mut input = create_input();
        input.variables = Some(variables);
        let error = save(&repository, input, 100).expect_err("malformed variables must fail");
        assert!(error.contains(expected), "{error}");
    }
    assert!(repository.list().expect("list commands").is_empty());
}

#[test]
fn update_requires_an_existing_id() {
    let database = Database::open_in_memory().expect("open database");
    let repository = CommandRepository::new(&database);
    let mut input = create_input();
    input.id = Some("missing".to_string());

    assert_eq!(
        save(&repository, input, 100).expect_err("missing update must fail"),
        "Saved command not found"
    );
}

#[test]
fn rejects_whitespace_only_command_text() {
    let database = Database::open_in_memory().expect("open database");
    let repository = CommandRepository::new(&database);
    let mut input = create_input();
    input.command_text = "  \n  ".to_string();

    assert!(save(&repository, input, 100)
        .expect_err("blank command must fail")
        .contains("Command text must contain"));
}
