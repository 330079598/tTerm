//! Startup forwarding does not assume paths, windows, or services are initialized.
use tterm_platform::launch::LaunchRequest;
/// Delivers the original arguments and working directory to the client's existing queue.
pub fn receive(request: LaunchRequest, enqueue: impl FnOnce(&[String], &str)) {
    enqueue(&request.args, &request.cwd.to_string_lossy());
}
#[cfg(test)]
mod tests {
    #[test]
    fn forwards_before_any_service_initialization() {
        let request = tterm_platform::launch::LaunchRequest {
            args: vec!["tTerm".into(), "a b".into()],
            cwd: "/tmp/中文".into(),
        };
        super::receive(request, |args, cwd| {
            assert_eq!(args[1], "a b");
            assert_eq!(cwd, "/tmp/中文");
        });
    }
}
