use std::sync::Arc;

use tterm_core::monitor::prompts::PendingMonitorPrompts;
use tterm_core::{
    core::state::PtyMap, core::state::ZmodemArmedSendMap, core::state::ZmodemManualDetectMap,
    core::state::ZmodemMap, monitor::types::MonitorSessionMap,
};

pub struct SessionServices {
    pub ptys: PtyMap,
    pub zmodem: ZmodemMap,
    pub zmodem_armed_send: ZmodemArmedSendMap,
    pub zmodem_manual_detect: ZmodemManualDetectMap,
    pub monitors: MonitorSessionMap,
    pub monitor_prompts: Arc<PendingMonitorPrompts>,
}
