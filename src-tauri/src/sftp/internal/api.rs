macro_rules! with_sftp {
    ($app:expr, $tab_id:expr, $plan:expr, $prompts:expr, $pool:expr, $sftp:ident => $body:block) => {{
        let key = $crate::sftp::internal::types::SftpConnectionKey {
            tab_id: $tab_id.to_string(),
            host: $plan.host.clone().ok_or("Host is required")?,
            port: $plan.port,
            username: $plan.username.clone().ok_or("Username is required")?,
        };

        // Ensure connection exists
        $crate::sftp::internal::connection::get_or_create_sftp_connection(
            $app,
            $tab_id,
            $plan,
            $prompts.clone(),
            $pool,
        )
        .await?;

        // Clone the session handle so long-running operations do not hold the pool lock.
        let sftp = {
            let pool_guard = $pool.read().await;
            let cached = pool_guard.get(&key).ok_or("Connection not found")?;
            cached.connection.sftp.clone()
        };

        let result: Result<_, String> = async {
            let $sftp = sftp.as_ref();
            $body
        }
        .await;

        // If operation fails, connection may be broken, remove from cache.
        if result.is_err() {
            let mut pool_guard = $pool.write().await;
            if let Some(cached) = pool_guard.remove(&key) {
                tokio::spawn(async move {
                    $crate::sftp::internal::connection::close_sftp(cached.connection).await;
                });
            }
        }

        result
    }};
}

pub mod base;
pub mod delete;
pub mod upload;
