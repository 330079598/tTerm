//! Distribution identity for the server monitor's icons.

use serde::{Deserialize, Serialize};

use crate::events::ssh::LinuxDistributionInfo;

/// The distributions with an icon of their own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DistroId {
    /// Generic Linux.
    Linux,
    /// Ubuntu.
    Ubuntu,
    /// Debian.
    Debian,
    /// CentOS.
    Centos,
    /// Red Hat Enterprise Linux.
    Rhel,
    /// Fedora.
    Fedora,
    /// Rocky Linux.
    Rocky,
    /// AlmaLinux.
    Alma,
    /// Oracle Linux.
    Oracle,
    /// Amazon Linux.
    Amazon,
    /// openSUSE.
    Suse,
    /// Alpine Linux.
    Alpine,
    /// Arch Linux.
    Arch,
    /// Manjaro.
    Manjaro,
    /// EndeavourOS.
    Endeavouros,
    /// CachyOS.
    Cachyos,
    /// Garuda Linux.
    Garuda,
    /// Gentoo.
    Gentoo,
    /// NixOS.
    Nixos,
    /// Void Linux.
    Void,
    /// Slackware.
    Slackware,
    /// Kali Linux.
    Kali,
    /// Parrot OS.
    Parrot,
    /// Linux Mint.
    Linuxmint,
    /// Pop!_OS.
    Popos,
    /// elementary OS.
    Elementary,
    /// Zorin OS.
    Zorin,
    /// deepin.
    Deepin,
    /// MX Linux.
    Mx,
    /// antiX.
    Antix,
    /// BigLinux.
    Biglinux,
    /// Nobara.
    Nobara,
    /// Bazzite.
    Bazzite,
    /// FreeBSD.
    Freebsd,
    /// macOS.
    Macos,
}

/// The distributions with an icon of their own, generic Linux first.
pub const DISTRO_IDS: [DistroId; 35] = [
    DistroId::Linux,
    DistroId::Ubuntu,
    DistroId::Debian,
    DistroId::Centos,
    DistroId::Rhel,
    DistroId::Fedora,
    DistroId::Rocky,
    DistroId::Alma,
    DistroId::Oracle,
    DistroId::Amazon,
    DistroId::Suse,
    DistroId::Alpine,
    DistroId::Arch,
    DistroId::Manjaro,
    DistroId::Endeavouros,
    DistroId::Cachyos,
    DistroId::Garuda,
    DistroId::Gentoo,
    DistroId::Nixos,
    DistroId::Void,
    DistroId::Slackware,
    DistroId::Kali,
    DistroId::Parrot,
    DistroId::Linuxmint,
    DistroId::Popos,
    DistroId::Elementary,
    DistroId::Zorin,
    DistroId::Deepin,
    DistroId::Mx,
    DistroId::Antix,
    DistroId::Biglinux,
    DistroId::Nobara,
    DistroId::Bazzite,
    DistroId::Freebsd,
    DistroId::Macos,
];

impl DistroId {
    /// The identifier used in icon file names and settings.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Linux => "linux",
            Self::Ubuntu => "ubuntu",
            Self::Debian => "debian",
            Self::Centos => "centos",
            Self::Rhel => "rhel",
            Self::Fedora => "fedora",
            Self::Rocky => "rocky",
            Self::Alma => "alma",
            Self::Oracle => "oracle",
            Self::Amazon => "amazon",
            Self::Suse => "suse",
            Self::Alpine => "alpine",
            Self::Arch => "arch",
            Self::Manjaro => "manjaro",
            Self::Endeavouros => "endeavouros",
            Self::Cachyos => "cachyos",
            Self::Garuda => "garuda",
            Self::Gentoo => "gentoo",
            Self::Nixos => "nixos",
            Self::Void => "void",
            Self::Slackware => "slackware",
            Self::Kali => "kali",
            Self::Parrot => "parrot",
            Self::Linuxmint => "linuxmint",
            Self::Popos => "popos",
            Self::Elementary => "elementary",
            Self::Zorin => "zorin",
            Self::Deepin => "deepin",
            Self::Mx => "mx",
            Self::Antix => "antix",
            Self::Biglinux => "biglinux",
            Self::Nobara => "nobara",
            Self::Bazzite => "bazzite",
            Self::Freebsd => "freebsd",
            Self::Macos => "macos",
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        DISTRO_IDS
            .into_iter()
            .find(|distro| distro.as_str() == value)
    }
}

/// The distribution whose icon a host shows; generic Linux for one without
/// its own. What the host says it is wins over what it says it is like, so
/// Linux Mint, which is like Ubuntu, shows its own.
pub fn detect_distro_id(info: Option<&LinuxDistributionInfo>) -> DistroId {
    let mut values = Vec::new();
    if let Some(info) = info {
        for value in [&info.id, &info.name, &info.pretty_name] {
            if !value.is_empty() {
                values.push(value.to_lowercase());
            }
        }
        for value in &info.id_like {
            if !value.is_empty() {
                values.push(value.to_lowercase());
            }
        }
    }

    for value in values {
        if let Some((_, id, _)) = RULES.iter().find(|(patterns, _, exact)| {
            patterns.iter().any(|pattern| {
                if *exact {
                    value == *pattern
                } else {
                    value.contains(pattern)
                }
            })
        }) {
            return *id;
        }
    }
    DistroId::Linux
}

/// What tTerm's WSL launcher reports from os-release,
/// `OSC 777 ; tterm-os ; <ID> ; <ID_LIKE> ; <NAME>`.
pub fn parse_os_report(data: &str) -> Option<LinuxDistributionInfo> {
    let fields: Vec<&str> = data.split(';').collect();
    if fields.len() > 4 {
        return None;
    }
    let command = fields.first().copied().unwrap_or_default();
    let id = fields.get(1).copied().unwrap_or_default();
    let id_like = fields.get(2).copied().unwrap_or_default();
    let name = fields.get(3).copied().unwrap_or_default();
    if command != "tterm-os" || (id.is_empty() && name.is_empty()) {
        return None;
    }
    Some(LinuxDistributionInfo {
        id: id.to_owned(),
        id_like: id_like
            .split(' ')
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .collect(),
        name: name.to_owned(),
        pretty_name: String::new(),
        version_id: None,
    })
}

/// Whether `value` is one of the distributions with an icon.
pub fn is_distro_id(value: &str) -> bool {
    DistroId::from_str(value).is_some()
}

/// The product name shown for a distribution.
pub fn distro_name(id: DistroId) -> &'static str {
    match id {
        DistroId::Linux => "Linux",
        DistroId::Ubuntu => "Ubuntu",
        DistroId::Debian => "Debian",
        DistroId::Centos => "CentOS",
        DistroId::Rhel => "Red Hat Enterprise Linux",
        DistroId::Fedora => "Fedora",
        DistroId::Rocky => "Rocky Linux",
        DistroId::Alma => "AlmaLinux",
        DistroId::Oracle => "Oracle Linux",
        DistroId::Amazon => "Amazon Linux",
        DistroId::Suse => "openSUSE",
        DistroId::Alpine => "Alpine Linux",
        DistroId::Arch => "Arch Linux",
        DistroId::Manjaro => "Manjaro",
        DistroId::Endeavouros => "EndeavourOS",
        DistroId::Cachyos => "CachyOS",
        DistroId::Garuda => "Garuda Linux",
        DistroId::Gentoo => "Gentoo",
        DistroId::Nixos => "NixOS",
        DistroId::Void => "Void Linux",
        DistroId::Slackware => "Slackware",
        DistroId::Kali => "Kali Linux",
        DistroId::Parrot => "Parrot OS",
        DistroId::Linuxmint => "Linux Mint",
        DistroId::Popos => "Pop!_OS",
        DistroId::Elementary => "elementary OS",
        DistroId::Zorin => "Zorin OS",
        DistroId::Deepin => "deepin",
        DistroId::Mx => "MX Linux",
        DistroId::Antix => "antiX",
        DistroId::Biglinux => "BigLinux",
        DistroId::Nobara => "Nobara",
        DistroId::Bazzite => "Bazzite",
        DistroId::Freebsd => "FreeBSD",
        DistroId::Macos => "macOS",
    }
}

/// The icon path for a distribution.
pub fn distro_icon_src(id: DistroId) -> String {
    format!("/assets/distro-icons/{}.svg", id.as_str())
}

type DistroRule = (&'static [&'static str], DistroId, bool);

const RULES: &[DistroRule] = &[
    (&["archlinux"], DistroId::Arch, false),
    (&["ubuntu"], DistroId::Ubuntu, false),
    (&["linuxmint", "linux mint"], DistroId::Linuxmint, false),
    (&["mint"], DistroId::Linuxmint, false),
    (&["debian"], DistroId::Debian, false),
    (&["gentoo"], DistroId::Gentoo, false),
    (&["manjaro"], DistroId::Manjaro, false),
    (&["kali"], DistroId::Kali, false),
    (&["pop!_os", "pop_os"], DistroId::Popos, false),
    (&["pop os", "pop!"], DistroId::Popos, false),
    (&["zorin"], DistroId::Zorin, false),
    (&["elementary"], DistroId::Elementary, false),
    (&["mx linux"], DistroId::Mx, false),
    (&["mx"], DistroId::Mx, true),
    (&["endeavouros"], DistroId::Endeavouros, false),
    (&["endeavour"], DistroId::Endeavouros, false),
    (&["cachyos"], DistroId::Cachyos, false),
    (&["nixos", "nix os"], DistroId::Nixos, false),
    (&["nobara"], DistroId::Nobara, false),
    (&["bazzite"], DistroId::Bazzite, false),
    (&["antix"], DistroId::Antix, false),
    (&["biglinux", "big linux"], DistroId::Biglinux, false),
    (&["deepin"], DistroId::Deepin, false),
    (&["garuda"], DistroId::Garuda, false),
    (&["slackware"], DistroId::Slackware, false),
    (&["void"], DistroId::Void, false),
    (&["parrot"], DistroId::Parrot, false),
    (&["rocky"], DistroId::Rocky, false),
    (&["alma"], DistroId::Alma, false),
    (&["centos"], DistroId::Centos, false),
    (&["rhel", "redhat", "red hat"], DistroId::Rhel, false),
    (&["fedora"], DistroId::Fedora, false),
    (&["suse", "opensuse"], DistroId::Suse, false),
    (&["arch"], DistroId::Arch, false),
    (&["alpine"], DistroId::Alpine, false),
    (&["amazon"], DistroId::Amazon, false),
    (&["amzn"], DistroId::Amazon, true),
    (&["oracle"], DistroId::Oracle, false),
    (&["freebsd"], DistroId::Freebsd, false),
    (&["macos", "mac os x"], DistroId::Macos, false),
];

#[cfg(test)]
mod tests;
