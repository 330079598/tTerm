use crate::core::state::{ChannelContext, TabZmodem};
use crate::session::sink::SessionSink;
use std::sync::Arc;

pub(crate) fn channel(sink: Arc<dyn SessionSink>) -> ChannelContext {
    ChannelContext {
        sink,
        tab_id: "p1-06-test".into(),
        session_nonce: 6,
        active: Default::default(),
        zmodem: TabZmodem {
            map: Default::default(),
            armed_send_map: Default::default(),
            manual_override: Default::default(),
        },
    }
}
