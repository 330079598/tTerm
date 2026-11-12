use super::{
    DISTRO_IDS, DistroId, detect_distro_id, distro_icon_src, distro_name, is_distro_id,
    parse_os_report,
};
use crate::events::ssh::LinuxDistributionInfo;
use serde_json::json;

fn info(id: &str, id_like: &[&str], name: &str, pretty_name: &str) -> LinuxDistributionInfo {
    LinuxDistributionInfo {
        id: id.to_owned(),
        id_like: id_like.iter().map(|value| (*value).to_owned()).collect(),
        name: name.to_owned(),
        pretty_name: pretty_name.to_owned(),
        version_id: None,
    }
}

#[test]
fn knows_a_distribution_by_its_id() {
    assert_eq!(
        detect_distro_id(Some(&info("ubuntu", &["debian"], "", ""))),
        DistroId::Ubuntu
    );
    assert_eq!(
        detect_distro_id(Some(&info("amzn", &[], "", ""))),
        DistroId::Amazon
    );
}

#[test]
fn prefers_a_derivative_over_the_distribution_it_is_based_on() {
    assert_eq!(
        detect_distro_id(Some(&info("linuxmint", &["ubuntu", "debian"], "", ""))),
        DistroId::Linuxmint
    );
    assert_eq!(
        detect_distro_id(Some(&info("manjaro", &["arch"], "", ""))),
        DistroId::Manjaro
    );
}

#[test]
fn falls_back_to_what_a_distribution_is_like() {
    assert_eq!(
        detect_distro_id(Some(&info("someos", &["rhel", "fedora"], "", ""))),
        DistroId::Rhel
    );
}

#[test]
fn knows_freebsd_by_its_os_release_and_macos_by_what_the_monitor_reports() {
    assert_eq!(
        detect_distro_id(Some(&info("freebsd", &[], "", "FreeBSD 14.1-RELEASE"))),
        DistroId::Freebsd
    );
    assert_eq!(
        detect_distro_id(Some(&info("macos", &[], "macOS", ""))),
        DistroId::Macos
    );
}

#[test]
fn shows_generic_linux_for_an_unknown_distribution_or_none() {
    assert_eq!(
        detect_distro_id(Some(&info("someos", &[], "Some OS", ""))),
        DistroId::Linux
    );
    assert_eq!(detect_distro_id(None), DistroId::Linux);
}

#[test]
fn reads_the_wsl_launchers_report() {
    assert_eq!(
        parse_os_report("tterm-os;ubuntu;debian;Ubuntu"),
        Some(info("ubuntu", &["debian"], "Ubuntu", ""))
    );
    assert_eq!(
        parse_os_report("tterm-os;rocky;rhel centos fedora;Rocky Linux")
            .map(|report| report.id_like),
        Some(vec![
            "rhel".to_owned(),
            "centos".to_owned(),
            "fedora".to_owned(),
        ])
    );
}

#[test]
fn names_pop_os_by_the_name_its_id_leaves_out() {
    assert_eq!(
        detect_distro_id(parse_os_report("tterm-os;pop;ubuntu debian;Pop!_OS").as_ref()),
        DistroId::Popos
    );
}

#[test]
fn ignores_other_osc777_payloads_and_empty_reports() {
    assert_eq!(parse_os_report("tterm-agent;claude;done"), None);
    assert_eq!(parse_os_report("notify;title;body"), None);
    assert_eq!(parse_os_report("tterm-os;;;"), None);
    assert_eq!(parse_os_report("tterm-os;ubuntu;debian;Ubuntu;extra"), None);
}

#[test]
fn accepts_only_distributions_with_an_icon() {
    assert!(is_distro_id("debian"));
    assert!(!is_distro_id("windows"));
    assert!(!is_distro_id(""));
    assert_eq!(distro_name(DistroId::Rhel), "Red Hat Enterprise Linux");
    assert_eq!(
        distro_icon_src(DistroId::Ubuntu),
        "/assets/distro-icons/ubuntu.svg"
    );
}

#[test]
fn every_distribution_has_the_name_and_icon_the_frontend_ships() {
    let expected = [
        ("linux", "Linux"),
        ("ubuntu", "Ubuntu"),
        ("debian", "Debian"),
        ("centos", "CentOS"),
        ("rhel", "Red Hat Enterprise Linux"),
        ("fedora", "Fedora"),
        ("rocky", "Rocky Linux"),
        ("alma", "AlmaLinux"),
        ("oracle", "Oracle Linux"),
        ("amazon", "Amazon Linux"),
        ("suse", "openSUSE"),
        ("alpine", "Alpine Linux"),
        ("arch", "Arch Linux"),
        ("manjaro", "Manjaro"),
        ("endeavouros", "EndeavourOS"),
        ("cachyos", "CachyOS"),
        ("garuda", "Garuda Linux"),
        ("gentoo", "Gentoo"),
        ("nixos", "NixOS"),
        ("void", "Void Linux"),
        ("slackware", "Slackware"),
        ("kali", "Kali Linux"),
        ("parrot", "Parrot OS"),
        ("linuxmint", "Linux Mint"),
        ("popos", "Pop!_OS"),
        ("elementary", "elementary OS"),
        ("zorin", "Zorin OS"),
        ("deepin", "deepin"),
        ("mx", "MX Linux"),
        ("antix", "antiX"),
        ("biglinux", "BigLinux"),
        ("nobara", "Nobara"),
        ("bazzite", "Bazzite"),
        ("freebsd", "FreeBSD"),
        ("macos", "macOS"),
    ];
    assert_eq!(DISTRO_IDS.len(), expected.len());
    for (id, name) in DISTRO_IDS.into_iter().zip(expected) {
        assert_eq!(id.as_str(), name.0);
        assert_eq!(distro_name(id), name.1);
        assert_eq!(
            distro_icon_src(id),
            format!("/assets/distro-icons/{}.svg", name.0)
        );
    }
}

#[test]
fn exact_rules_and_names_from_os_release_choose_the_expected_icon() {
    assert_eq!(
        detect_distro_id(Some(&info("mx", &[], "", ""))),
        DistroId::Mx
    );
    assert_eq!(
        detect_distro_id(Some(&info("someos", &[], "MX Linux", ""))),
        DistroId::Mx
    );
    assert_eq!(
        detect_distro_id(Some(&info("someos", &[], "Arch Linux", ""))),
        DistroId::Arch
    );
    assert_eq!(
        detect_distro_id(Some(&info("someos", &[], "Linux Mint", ""))),
        DistroId::Linuxmint
    );
    assert_eq!(
        parse_os_report("tterm-os;;;name").map(|report| (report.id, report.name)),
        Some((String::new(), "name".to_owned()))
    );
    assert_eq!(parse_os_report("tterm-os"), None);
}

#[test]
fn serializes_distribution_ids_and_reports_like_the_frontend_and_backend() {
    for id in DISTRO_IDS {
        let json = serde_json::to_string(&id).expect("serialize a distribution id");
        assert_eq!(json, format!("\"{}\"", id.as_str()));
        assert_eq!(
            serde_json::from_str::<DistroId>(&json).expect("deserialize a distribution id"),
            id
        );
    }
    assert_eq!(
        serde_json::to_value(info("ubuntu", &["debian"], "Ubuntu", ""))
            .expect("serialize a distribution report"),
        json!({
            "id": "ubuntu",
            "idLike": ["debian"],
            "name": "Ubuntu",
            "prettyName": "",
            "versionId": null
        })
    );
}
