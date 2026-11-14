use std::sync::Arc;

use tterm_core::sftp::internal::api::external::ExternalEditState;
use tterm_core::{
    sftp::internal::types::SftpConnectionPool, sftp::internal::types::TransferCancelMap,
};

pub struct SftpServices {
    pub pool: SftpConnectionPool,
    pub transfer_cancel: TransferCancelMap,
    pub external_edit: Arc<ExternalEditState>,
}
