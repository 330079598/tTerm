use std::sync::Arc;

use crate::{
    core::state::{PtyMap, ZmodemArmedSendMap, ZmodemManualDetectMap, ZmodemMap},
    monitor::{prompts::PendingMonitorPrompts, types::MonitorSessionMap},
};

pub struct SessionServices {
    pub ptys: PtyMap,
    pub zmodem: ZmodemMap,
    pub zmodem_armed_send: ZmodemArmedSendMap,
    pub zmodem_manual_detect: ZmodemManualDetectMap,
    pub monitors: MonitorSessionMap,
    pub monitor_prompts: Arc<PendingMonitorPrompts>,
}
