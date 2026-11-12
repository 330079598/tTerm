use super::{
    ProxyServer, ProxyServerEntry, SaveProxyInput, delete_proxy_service, list_proxies_service,
    save_proxy_service,
};

#[tauri::command]
pub async fn list_proxies(
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<Vec<ProxyServerEntry>, String> {
    let secret_state = services.shared.secrets.clone();
    list_proxies_service(secret_state).await
}

/// Adds or updates a proxy server. `password` replaces the saved one when
/// given; `clear_password` removes it, as does a server without a user.
#[tauri::command]
pub async fn save_proxy(
    server: ProxyServer,
    password: Option<String>,
    clear_password: bool,
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<Vec<ProxyServerEntry>, String> {
    let secret_state = services.shared.secrets.clone();
    save_proxy_service(
        secret_state,
        SaveProxyInput {
            server,
            password,
            clear_password,
        },
    )
    .await
}

/// Removes a proxy server and its password. Hosts that chose it keep the
/// choice and report the proxy as missing when they connect.
#[tauri::command]
pub async fn delete_proxy(
    id: String,
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<Vec<ProxyServerEntry>, String> {
    let secret_state = services.shared.secrets.clone();
    delete_proxy_service(secret_state, id).await
}
