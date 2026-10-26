use super::*;
use crate::db::Database;
use crate::profiles::storage::get_profile;

fn profile(id: &str, group_id: Option<&str>) -> SavedProfile {
    serde_json::from_value(serde_json::json!({
        "id": id,
        "name": id.to_uppercase(),
        "group_id": group_id,
        "connection_type": "ssh",
        "host": "example.com",
    }))
    .expect("profile fixture")
}

fn group(id: &str, name: &str, parent_id: Option<&str>) -> ProfileGroup {
    ProfileGroup {
        id: id.to_string(),
        name: name.to_string(),
        parent_id: parent_id.map(str::to_string),
    }
}

fn group_id_of(connection: &Connection, profile_id: &str) -> Option<String> {
    get_profile(connection, profile_id)
        .expect("read profile")
        .expect("profile exists")
        .group_id
}

fn parent_of(connection: &Connection, id: &str) -> Option<String> {
    list_groups(connection)
        .expect("list groups")
        .into_iter()
        .find(|group| group.id == id)
        .expect("group exists")
        .parent_id
}

fn with_database(test: impl FnOnce(&Connection) -> Result<(), String>) {
    let database = Database::open_in_memory().expect("open database");
    database.write(|connection| test(connection)).expect("test");
}

#[test]
fn groups_nest_and_show_their_path_from_the_top() {
    with_database(|connection| {
        let prod = create_group(connection, " prod ", None)?;
        let web = create_group(connection, "web", Some(&prod.id))?;
        create_group(connection, "web", None)?;
        assert_eq!(prod.name, "prod");

        let paths = group_paths(&list_groups(connection)?);
        assert_eq!(paths[&web.id], "prod / web");
        assert_eq!(paths[&prod.id], "prod");
        Ok(())
    });
}

#[test]
fn sibling_names_stay_distinct_and_free_of_the_separator() {
    with_database(|connection| {
        let prod = create_group(connection, "prod", None)?;
        let dev = create_group(connection, "dev", None)?;
        create_group(connection, "web", Some(&prod.id))?;
        let web = create_group(connection, "web", Some(&dev.id))?;

        assert!(create_group(connection, "prod", None).is_err());
        assert!(create_group(connection, " ", None).is_err());
        assert!(create_group(connection, "a/b", None).is_err());
        assert!(create_group(connection, "x", Some("missing")).is_err());
        assert!(rename_group(connection, &dev.id, "prod").is_err());
        assert!(move_group(connection, &web.id, Some(&prod.id)).is_err());

        rename_group(connection, &dev.id, "staging")?;
        rename_group(connection, &dev.id, "staging")?;
        let paths = group_paths(&list_groups(connection)?);
        assert_eq!(paths[&web.id], "staging / web");
        Ok(())
    });
}

#[test]
fn a_group_cannot_move_into_itself_or_a_group_inside_it() {
    with_database(|connection| {
        let a = create_group(connection, "a", None)?;
        let b = create_group(connection, "b", Some(&a.id))?;
        let c = create_group(connection, "c", Some(&b.id))?;

        assert!(move_group(connection, &a.id, Some(&a.id)).is_err());
        assert!(move_group(connection, &a.id, Some(&c.id)).is_err());

        move_group(connection, &c.id, None)?;
        assert_eq!(parent_of(connection, &c.id), None);
        move_group(connection, &a.id, Some(&c.id))?;
        assert_eq!(parent_of(connection, &a.id), Some(c.id.clone()));
        Ok(())
    });
}

#[test]
fn deleting_a_group_moves_what_it_holds_up_a_level() {
    with_database(|connection| {
        let prod = create_group(connection, "prod", None)?;
        let web = create_group(connection, "web", Some(&prod.id))?;
        let api = create_group(connection, "api", Some(&web.id))?;
        upsert_profile(connection, &profile("a", Some(&web.id)))?;
        upsert_profile(connection, &profile("b", Some(&api.id)))?;
        upsert_profile(connection, &profile("c", Some(&prod.id)))?;

        delete_group(connection, &web.id)?;
        assert_eq!(group_id_of(connection, "a"), Some(prod.id.clone()));
        assert_eq!(group_id_of(connection, "b"), Some(api.id.clone()));
        assert_eq!(parent_of(connection, &api.id), Some(prod.id.clone()));

        delete_group(connection, &prod.id)?;
        assert_eq!(group_id_of(connection, "a"), None);
        assert_eq!(group_id_of(connection, "c"), None);
        assert_eq!(parent_of(connection, &api.id), None);
        assert!(delete_group(connection, &prod.id).is_err());
        Ok(())
    });
}

#[test]
fn a_path_finds_its_groups_and_creates_the_missing_ones() {
    with_database(|connection| {
        let prod = create_group(connection, "prod", None)?;
        let web = resolve_path(connection, " prod /web/ ")?.expect("group");
        assert_eq!(parent_of(connection, &web), Some(prod.id.clone()));
        assert_eq!(resolve_path(connection, "prod / web")?, Some(web.clone()));
        assert_eq!(resolve_path(connection, "prod")?, Some(prod.id.clone()));
        assert_eq!(resolve_path(connection, " / ")?, None);
        assert_eq!(list_groups(connection)?.len(), 2);
        Ok(())
    });
}

#[test]
fn repair_mends_missing_parents_loops_and_profiles_in_deleted_groups() {
    with_database(|connection| {
        replace_groups(
            connection,
            &[
                group("orphan", "orphan", Some("gone")),
                group("self", "self", Some("self")),
                group("b", "b", Some("c")),
                group("c", "c", Some("b")),
                group("d", "d", Some("c")),
            ],
        )?;
        upsert_profile(connection, &profile("kept", Some("d")))?;
        upsert_profile(connection, &profile("lost", Some("gone")))?;

        repair(connection)?;
        assert_eq!(parent_of(connection, "orphan"), None);
        assert_eq!(parent_of(connection, "self"), None);
        assert_eq!(parent_of(connection, "b"), None);
        assert_eq!(parent_of(connection, "c"), Some("b".to_string()));
        assert_eq!(parent_of(connection, "d"), Some("c".to_string()));
        assert_eq!(group_id_of(connection, "kept"), Some("d".to_string()));
        assert_eq!(group_id_of(connection, "lost"), None);
        Ok(())
    });
}

#[test]
fn groups_read_from_older_versions_are_keyed_by_their_name() {
    let groups: Vec<ProfileGroup> = serde_json::from_value(serde_json::json!([
        "ops",
        { "id": "g1", "name": "web", "parentId": "ops" },
        { "id": "g2", "name": "db" },
    ]))
    .expect("groups");
    assert_eq!(
        groups,
        [
            group("ops", "ops", None),
            group("g1", "web", Some("ops")),
            group("g2", "db", None)
        ]
    );

    let legacy: SavedProfile = serde_json::from_value(serde_json::json!({
        "id": "a", "name": "A", "group": " dev ", "connection_type": "ssh"
    }))
    .expect("profile");
    let filed = profile("b", Some("ops"));
    let groups = with_legacy_groups(groups, &[legacy, filed]);
    assert_eq!(groups.len(), 4);
    assert_eq!(groups[3], group("dev", "dev", None));
}
