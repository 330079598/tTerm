use super::handle_os_report;
use crate::distro::DistroId;

#[test]
fn names_the_distribution_a_local_wsl_shell_reports() {
    assert_eq!(
        handle_os_report("tterm-os;kali;debian;Kali GNU/Linux", false),
        Some(DistroId::Kali)
    );
}

#[test]
fn shows_generic_linux_for_a_distribution_without_a_logo() {
    assert_eq!(
        handle_os_report("tterm-os;someos;;Some OS", false),
        Some(DistroId::Linux)
    );
}

#[test]
fn ignores_reports_over_ssh_and_other_osc_777_payloads() {
    assert_eq!(
        handle_os_report("tterm-os;ubuntu;debian;Ubuntu", true),
        None
    );
    assert_eq!(handle_os_report("notify;title;body", false), None);
}
