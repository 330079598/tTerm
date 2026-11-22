//! Reads the tunnel page snapshot through the existing core services.

use std::collections::HashMap;
use std::sync::Arc;

use tterm_core::events::tunnel::{TunnelState, TunnelStatus, TunnelTraffic};
use tterm_core::profiles::launch::{SavedSshProfileSummary, list_saved_ssh_profile_summaries};
use tterm_core::services::AppServices;
use tterm_core::tunnel::storage::{list_tunnel_rules, list_tunnel_traffic};
use tterm_core::tunnel::types::TunnelRule;

use super::{TunnelPageModel, TunnelPageState, TunnelRow, host_label};

/// The raw read results for one page snapshot.
#[derive(Debug, Clone)]
pub(super) struct TunnelPageSnapshot {
    pub rules: Vec<TunnelRule>,
    pub statuses: Vec<TunnelStatus>,
    pub profiles: Vec<SavedSshProfileSummary>,
}

/// Reads rules, live manager statuses and display-only profile metadata.
pub(super) async fn load_snapshot(
    services: Arc<AppServices>,
) -> Result<TunnelPageSnapshot, String> {
    let runtime = services.shared.runtime.clone();
    let manager = Arc::clone(&services.app.tunnels);
    runtime
        .spawn(async move {
            let (rules, saved, profiles) = tokio::task::spawn_blocking(|| {
                tterm_core::db::read(|connection| {
                    Ok((
                        list_tunnel_rules(connection)?,
                        list_tunnel_traffic(connection)?,
                        list_saved_ssh_profile_summaries(connection)?,
                    ))
                })
            })
            .await
            .map_err(|error| format!("Failed to read tunnels: {error}"))??;
            Ok(TunnelPageSnapshot {
                statuses: manager.statuses_for(&rules, &saved),
                rules,
                profiles,
            })
        })
        .await
        .map_err(|error| format!("Failed to load tunnels: {error}"))?
}

/// Joins the saved rules with their statuses and display-only host metadata.
pub(super) fn build_model(snapshot: TunnelPageSnapshot) -> TunnelPageModel {
    let mut statuses = snapshot
        .statuses
        .into_iter()
        .map(|status| (status.id.clone(), status))
        .collect::<HashMap<_, _>>();
    let profiles = snapshot
        .profiles
        .into_iter()
        .map(|profile| (profile.id.clone(), profile))
        .collect::<HashMap<_, _>>();
    let rules = snapshot
        .rules
        .into_iter()
        .map(|rule| {
            let status = statuses
                .remove(&rule.id)
                .unwrap_or_else(|| TunnelStatus::stopped(&rule.id, TunnelTraffic::default()));
            let profile = profiles.get(&rule.profile_id);
            TunnelRow {
                host_label: host_label(profile),
                host_missing: profile.is_none(),
                rule,
                status,
            }
        })
        .collect::<Vec<_>>();
    let running_count = rules
        .iter()
        .filter(|row| row.status.state == TunnelState::Running)
        .count();
    TunnelPageModel {
        state: TunnelPageState::Ready,
        rules,
        running_count,
    }
}
