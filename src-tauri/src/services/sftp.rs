use std::sync::Arc;

use crate::sftp::internal::{
    api::external::ExternalEditState,
    types::{SftpConnectionPool, TransferCancelMap},
};

pub struct SftpServices {
    pub pool: SftpConnectionPool,
    pub transfer_cancel: TransferCancelMap,
    pub external_edit: Arc<ExternalEditState>,
}
