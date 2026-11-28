//! Recent saved connections for one opening of the new-tab menu.

use std::sync::Arc;

use std::collections::HashMap;

use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::{App, Context, Entity, PromptLevel, Task, WeakEntity, Window};
use tterm_core::connections::profile_recents::RecentProfile;
use tterm_core::i18n::{keys, translate::TranslationKey};
use tterm_core::profiles::launch::SavedSshProfileSummary;
use tterm_core::services::AppServices;

use super::{Locale, Prototype};

type Profiles = Vec<SavedSshProfileSummary>;

fn recently_used(profiles: Profiles, mut recents: Vec<RecentProfile>) -> Profiles {
    let mut profiles: HashMap<_, _> = profiles
        .into_iter()
        .map(|row| (row.id.clone(), row))
        .collect();
    recents.sort_by_key(|row| std::cmp::Reverse(row.last_used_at));
    recents
        .into_iter()
        .filter_map(|row| profiles.remove(&row.profile_id))
        .take(8)
        .collect()
}

pub(super) fn text(locale: &Entity<Locale>, key: TranslationKey, cx: &App) -> String {
    locale
        .read(cx)
        .text(key)
        .unwrap_or_else(|_| key.as_str().to_owned())
}

/// Rebuilds only the child menu when the read finishes, leaving the shell selection intact.
pub(super) fn submenu(
    menu: PopupMenu,
    owner: &WeakEntity<Prototype>,
    locale: &Entity<Locale>,
    services: &Arc<AppServices>,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let label = text(locale, keys::QUICK_CONNECT_SECTIONS_RECENT, cx);
    let (owner, locale, services) = (owner.clone(), locale.clone(), services.clone());
    menu.submenu(label, window, cx, move |menu, window, cx| {
        let loading = text(&locale, keys::SAVED_CONNECTIONS_LOADING, cx);
        let (owner, locale) = (owner.clone(), locale.clone());
        let task = load(&services, cx);
        cx.spawn_in(window, async move |child, cx| {
            let result = task.await;
            let _ = child.update_in(cx, |child, window, cx| {
                child.rebuild(window, cx, |mut menu, _, cx| {
                    let rows = match result {
                        Ok(rows) if rows.is_empty() => {
                            return menu.label(text(
                                &locale,
                                keys::SAVED_CONNECTIONS_NO_RECENT,
                                cx,
                            ));
                        }
                        Ok(rows) => rows,
                        Err(error) => {
                            eprintln!("tterm-gpui: failed to load recent connections: {error}");
                            return menu.label(text(
                                &locale,
                                keys::SAVED_CONNECTIONS_LOAD_FAILED,
                                cx,
                            ));
                        }
                    };
                    for row in rows {
                        let owner = owner.clone();
                        let item = PopupMenuItem::new(format!("{} · {}", row.name, row.endpoint()));
                        menu = menu.item(item.on_click(move |_, window, app| {
                            let _ = owner
                                .update(app, |this, cx| confirm(this, row.id.clone(), window, cx));
                        }));
                    }
                    menu
                });
            });
        })
        .detach();
        menu.label(loading)
    })
}

#[cfg(not(test))]
fn load(services: &AppServices, cx: &mut Context<PopupMenu>) -> Task<Result<Profiles, String>> {
    use tterm_core::connections::profile_recents::read_recent_profiles;
    use tterm_core::profiles::launch::load_saved_ssh_profile_summaries;

    let task = services.shared.runtime.spawn_blocking(|| {
        let recents = read_recent_profiles()?;
        Ok(recently_used(load_saved_ssh_profile_summaries()?, recents))
    });
    cx.spawn(async move |_, _| task.await.map_err(|error| error.to_string())?)
}

#[cfg(test)]
fn load(_: &AppServices, cx: &mut Context<PopupMenu>) -> Task<Result<Profiles, String>> {
    Task::ready(Ok(cx
        .try_global::<Fixture>()
        .map(|data| recently_used(data.rows.clone(), data.recents.clone()))
        .unwrap_or_default()))
}

pub(super) fn confirm(
    this: &mut Prototype,
    id: String,
    window: &mut Window,
    cx: &mut Context<Prototype>,
) {
    #[cfg(test)]
    if cx.has_global::<Fixture>() {
        cx.global_mut::<Fixture>().confirmed = Some(id);
        return;
    }
    let task = this.open_saved_profile(id, window, cx);
    cx.spawn_in(window, async move |this, cx| {
        if let Err(error) = task.await {
            eprintln!("tterm-gpui: failed to open recent connection: {error}");
            let _ = this.update_in(cx, |this, window, cx| {
                let message = text(&this.ui.locale, keys::SAVED_CONNECTIONS_OPEN_FAILED, cx);
                let ok = text(&this.ui.locale, keys::COMMON_CLOSE, cx);
                drop(window.prompt(PromptLevel::Warning, &message, None, &[ok.as_str()], cx));
            });
        }
    })
    .detach();
}

#[cfg(test)]
pub(in super::super) struct Fixture {
    rows: Profiles,
    recents: Vec<RecentProfile>,
    confirmed: Option<String>,
}

#[cfg(test)]
impl gpui_kit::Global for Fixture {}

#[cfg(test)]
impl Fixture {
    pub(super) fn profiles(cx: &App) -> Profiles {
        cx.try_global::<Self>()
            .map(|data| data.rows.clone())
            .unwrap_or_default()
    }
    pub(super) fn recents(cx: &App) -> Vec<RecentProfile> {
        cx.try_global::<Self>()
            .map(|data| data.recents.clone())
            .unwrap_or_default()
    }
    pub(in super::super) fn install(cx: &mut App, rows: Profiles) {
        let recents = rows
            .iter()
            .map(|row| RecentProfile {
                profile_id: row.id.clone(),
                last_used_at: 1,
            })
            .collect();
        cx.set_global(Self {
            rows,
            recents,
            confirmed: None,
        });
    }
    pub(in super::super) fn install_with_recents(
        cx: &mut App,
        rows: Profiles,
        recents: Vec<RecentProfile>,
    ) {
        cx.set_global(Self {
            rows,
            recents,
            confirmed: None,
        });
    }
    pub(in super::super) fn confirmed(cx: &App) -> Option<String> {
        cx.try_global::<Self>()
            .and_then(|data| data.confirmed.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newest_eight_surviving_distinct_profiles_keep_their_stable_ids() {
        let profiles = (0..12)
            .map(|n| SavedSshProfileSummary {
                id: n.to_string(),
                name: "same".into(),
                host: None,
                port: None,
                username: None,
                group_path: None,
            })
            .collect();
        let recent = |id: usize, time| RecentProfile {
            profile_id: id.to_string(),
            last_used_at: time,
        };
        let mut history = vec![recent(99, 200), recent(2, 90), recent(4, 90), recent(2, 89)];
        history.extend((0..10).map(|n| recent(n, n as i64)));
        let ids: Vec<_> = recently_used(profiles, history)
            .into_iter()
            .map(|row| row.id)
            .collect();
        assert_eq!(ids, ["2", "4", "9", "8", "7", "6", "5", "3"]);
    }

    #[test]
    fn recent_submenu_excludes_unused_saved_profiles() {
        let web = SavedSshProfileSummary {
            id: "web".into(),
            name: "Production web".into(),
            username: Some("deploy".into()),
            host: Some("web.example".into()),
            port: Some(22),
            group_path: Some("Production".into()),
        };
        let db = SavedSshProfileSummary {
            id: "db".into(),
            name: "Database".into(),
            username: Some("root".into()),
            host: Some("db.example".into()),
            port: Some(22),
            group_path: Some("Production".into()),
        };
        let recents = vec![RecentProfile {
            profile_id: "web".into(),
            last_used_at: 2,
        }];
        let rows = recently_used(vec![web, db], recents);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "web");
    }
}
