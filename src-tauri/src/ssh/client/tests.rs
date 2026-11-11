use super::setup::latest_queued_size;
use tokio::sync::mpsc;

#[test]
fn latest_queued_size_keeps_the_newest_resize() {
    let (resize_tx, mut resize_rx) = mpsc::unbounded_channel();
    assert_eq!(latest_queued_size(&mut resize_rx, (24, 80)), (24, 80));

    resize_tx.send((30, 100)).unwrap();
    resize_tx.send((40, 120)).unwrap();
    assert_eq!(latest_queued_size(&mut resize_rx, (24, 80)), (40, 120));
    assert!(resize_rx.try_recv().is_err());
}
