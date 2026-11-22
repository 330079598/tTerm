//! Loads and opens confirmed saved SSH profiles without blocking the window.

use gpui_kit::{Context, Task, Window};
use tterm_core::profiles::launch::{
    SavedSshLaunch, SavedSshLaunchError, SavedSshProfileSummary, load_saved_ssh_launch,
    load_saved_ssh_profile_summaries,
};
use tterm_core::services::AppServices;

use super::{Prototype, TabLaunch};

const ALREADY_OPENING: &str = "That saved connection is already opening.";

/// Reads only display metadata on the blocking pool.
pub(super) async fn load_saved_ssh_profiles(
    services: &AppServices,
) -> Result<Vec<SavedSshProfileSummary>, String> {
    services
        .shared
        .runtime
        .spawn_blocking(load_saved_ssh_profile_summaries)
        .await
        .map_err(|error| format!("Failed to read saved SSH profiles: {error}"))?
}

impl Prototype {
    /// Loads one exact saved profile and, when it is ready, opens a new tab.
    ///
    /// The pending marker is reserved before the background task starts, so
    /// repeated confirmation cannot start two loads or two tabs.
    pub(super) fn open_saved_profile(
        &mut self,
        profile_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Task<Result<(), String>> {
        if !self.pending_profile_opens.insert(profile_id.clone()) {
            return Task::ready(Err(ALREADY_OPENING.to_string()));
        }
        let services = self.factory.sessions.services();
        let pending_id = profile_id.clone();
        cx.spawn_in(window, async move |prototype, cx| {
            let result = load_launch(&services, profile_id).await;
            let updated = cx.update(|window, app| {
                prototype.update(app, |prototype, cx| {
                    prototype.pending_profile_opens.remove(&pending_id);
                    match result {
                        Ok(launch) => prototype
                            .open_saved_profile_launch(launch, window, cx)
                            .map(|_| ())
                            .ok_or_else(|| {
                                "The saved connection could not open a new tab.".to_string()
                            }),
                        Err(error) => Err(error),
                    }
                })
            });
            match updated {
                Ok(Ok(result)) => result,
                Ok(Err(error)) => Err(error.to_string()),
                Err(_) => {
                    Err("The workspace closed before the saved connection opened.".to_string())
                }
            }
        })
    }

    /// Opens a prepared launch in one new tab without changing existing panes.
    pub(super) fn open_saved_profile_launch(
        &mut self,
        launch: SavedSshLaunch,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<usize> {
        self.tabs.next_id()?;
        if !self.close_settings(window, cx) {
            return None;
        }
        let previous = self.visible_ids();
        let saved_id = self.next_saved_id();
        self.tabs.push(|id| {
            self.factory.open_launch(
                id,
                TabLaunch::saved_profile(saved_id, launch),
                &self.ui.locale,
                cx,
            )
        });
        let new_id = self.tabs.active_id();
        self.splits.open(new_id);
        self.refresh_panes(previous, window, cx);
        self.mark_workspace_dirty(cx);
        Some(new_id)
    }
}

async fn load_launch(services: &AppServices, profile_id: String) -> Result<SavedSshLaunch, String> {
    services
        .shared
        .runtime
        .spawn_blocking(move || load_saved_ssh_launch(&profile_id))
        .await
        .map_err(|error| format!("Failed to read the saved SSH profile: {error}"))?
        .map_err(saved_launch_error)
}

fn saved_launch_error(error: SavedSshLaunchError) -> String {
    match error {
        SavedSshLaunchError::NotFound => {
            "The saved connection no longer exists. Choose another saved connection.".to_string()
        }
        SavedSshLaunchError::NotSsh => {
            "The selected saved connection is not an SSH connection.".to_string()
        }
        SavedSshLaunchError::Unavailable(error) => error,
    }
}

#[cfg(test)]
mod tests {
    use super::saved_launch_error;
    use tterm_core::profiles::launch::SavedSshLaunchError;

    #[test]
    fn launch_errors_say_what_the_user_can_do_next() {
        assert!(saved_launch_error(SavedSshLaunchError::NotFound).contains("no longer exists"));
        assert!(saved_launch_error(SavedSshLaunchError::NotSsh).contains("not an SSH"));
        assert_eq!(
            saved_launch_error(SavedSshLaunchError::Unavailable(
                "profile data is corrupt".to_string()
            )),
            "profile data is corrupt"
        );
    }
}
