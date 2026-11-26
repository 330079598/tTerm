use super::{
    PROFILE_RECENTS_LIMIT, PROFILE_RECENTS_META_KEY, RecentProfile, meta,
    read_recent_profiles_from, record_recent_profile_at,
};
use crate::db::Database;

fn seeded_recents(newest: i64) -> Vec<RecentProfile> {
    (0..PROFILE_RECENTS_LIMIT)
        .map(|index| RecentProfile {
            profile_id: format!("p{index}"),
            last_used_at: newest - index as i64,
        })
        .collect()
}

fn seed(database: &Database, recents: &[RecentProfile]) {
    database
        .write(|connection| meta::set(connection, PROFILE_RECENTS_META_KEY, recents))
        .unwrap();
}

fn assert_newest_first(recents: &[RecentProfile]) {
    assert!(recents.len() <= PROFILE_RECENTS_LIMIT);
    assert!(
        recents
            .windows(2)
            .all(|pair| pair[0].last_used_at >= pair[1].last_used_at)
    );
}

#[test]
fn an_older_fifty_first_profile_does_not_evict_the_newest_fifty() {
    let database = Database::open_in_memory().expect("open in-memory database");
    let stored = seeded_recents(PROFILE_RECENTS_LIMIT as i64);
    seed(&database, &stored);

    database
        .write(|connection| record_recent_profile_at(connection, "late", 0))
        .unwrap();

    let recents = database.read(read_recent_profiles_from).unwrap();
    assert_eq!(recents, stored);
    assert_eq!(recents.len(), PROFILE_RECENTS_LIMIT);
    assert!(recents.iter().any(|recent| recent.profile_id == "p49"));
    assert!(!recents.iter().any(|recent| recent.profile_id == "late"));
    assert_newest_first(&recents);
}

#[test]
fn a_newer_profile_still_evicts_the_oldest_one() {
    let database = Database::open_in_memory().expect("open in-memory database");
    let stored = seeded_recents(PROFILE_RECENTS_LIMIT as i64);
    seed(&database, &stored);

    database
        .write(|connection| record_recent_profile_at(connection, "newest", 100))
        .unwrap();

    let recents = database.read(read_recent_profiles_from).unwrap();
    assert_eq!(recents.len(), PROFILE_RECENTS_LIMIT);
    assert_eq!(
        recents.first(),
        Some(&RecentProfile {
            profile_id: "newest".to_owned(),
            last_used_at: 100,
        })
    );
    assert!(recents.iter().any(|recent| recent.profile_id == "p0"));
    assert!(!recents.iter().any(|recent| recent.profile_id == "p49"));
    assert_newest_first(&recents);
}

#[test]
fn a_delayed_use_of_an_existing_profile_cannot_regress_its_last_use() {
    let database = Database::open_in_memory().expect("open in-memory database");
    let stored = seeded_recents(PROFILE_RECENTS_LIMIT as i64);
    seed(&database, &stored);

    database
        .write(|connection| record_recent_profile_at(connection, "p0", 10))
        .unwrap();
    database
        .write(|connection| record_recent_profile_at(connection, "p49", 0))
        .unwrap();

    let recents = database.read(read_recent_profiles_from).unwrap();
    assert_eq!(recents, stored);
    assert_eq!(recents.len(), PROFILE_RECENTS_LIMIT);
    assert_eq!(
        recents.first(),
        Some(&RecentProfile {
            profile_id: "p0".to_owned(),
            last_used_at: PROFILE_RECENTS_LIMIT as i64,
        })
    );
    assert!(!recents.iter().any(|recent| recent.last_used_at < 1));
    assert_newest_first(&recents);
}
