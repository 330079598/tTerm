// The receive direction is wired (M2/M3); several send-direction items
// (`begin_send`, `send_chunk`, `finish_send`, the `Send*` engine states, a
// few frame types/constants) stay unused until the send milestone (M4/M5)
// gives them a real caller. Drop this once that wiring lands.

pub mod consts;
pub mod crc;
pub mod engine;
pub mod frame;

pub use consts::ZmodemDirection;
#[allow(unused_imports)]
pub use engine::ZmodemFeedResult;
pub use engine::{ZmodemAction, ZmodemEngine};
