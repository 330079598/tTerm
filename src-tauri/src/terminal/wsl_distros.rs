//! The WSL distributions installed for the current user, read from the
//! registry key WSL keeps them under, as Windows Terminal does. `wsl.exe
//! --list` says the same, but in UTF-16 whatever the console's code page,
//! with a localized marker on the default, and only once WSL answers.

use windows::core::{w, HSTRING, PCWSTR, PWSTR};
use windows::Win32::System::Registry::{
    RegCloseKey, RegEnumKeyExW, RegGetValueW, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER, KEY_READ,
    RRF_RT_REG_DWORD, RRF_RT_REG_SZ,
};

const LXSS_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Lxss";

/// The `State` of a distribution that is ready to start; others are still
/// being installed, converted or removed.
const STATE_INSTALLED: u32 = 1;

/// Docker Desktop's own distributions, which run its engine rather than a shell.
const DOCKER_DISTROS: [&str; 2] = ["docker-desktop", "docker-desktop-data"];

/// A distribution a local tab can start with `wsl.exe -d <name>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct WslDistro {
    pub name: String,
    /// The one `wsl.exe` starts when no distribution is named.
    pub is_default: bool,
}

/// A distribution as the registry lists it, under its GUID.
#[derive(Debug)]
struct RegisteredDistro {
    id: String,
    name: String,
    state: Option<u32>,
}

/// The distributions a local tab can start, the default first; none when
/// WSL has none or the registry can't be read.
pub(super) fn installed_distros() -> Vec<WslDistro> {
    let mut lxss = HKEY::default();
    // SAFETY: `lxss` outlives the call, which writes the opened key to it.
    let opened = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            &HSTRING::from(LXSS_KEY),
            None,
            KEY_READ,
            &mut lxss,
        )
    };
    if opened.is_err() {
        return Vec::new();
    }
    let default_id = read_string(lxss, PCWSTR::null(), w!("DefaultDistribution"));
    let registered = registered_distros(lxss);
    // SAFETY: the key was opened above and is closed only here.
    let _ = unsafe { RegCloseKey(lxss) };
    listed_distros(registered, default_id.as_deref())
}

/// The distributions under the open `Lxss` key, one per subkey.
fn registered_distros(lxss: HKEY) -> Vec<RegisteredDistro> {
    let mut distros = Vec::new();
    for index in 0.. {
        // A GUID in braces is 38 characters.
        let mut buffer = [0u16; 64];
        let mut length = buffer.len() as u32;
        // SAFETY: `buffer` holds `length` characters, the most the call writes.
        let status = unsafe {
            RegEnumKeyExW(
                lxss,
                index,
                Some(PWSTR(buffer.as_mut_ptr())),
                &mut length,
                None,
                None,
                None,
                None,
            )
        };
        if status.is_err() {
            break;
        }
        let id = String::from_utf16_lossy(&buffer[..length as usize]);
        let subkey = HSTRING::from(id.as_str());
        let Some(name) = read_string(lxss, &subkey, w!("DistributionName")) else {
            continue;
        };
        let state = read_dword(lxss, &subkey, w!("State"));
        distros.push(RegisteredDistro { id, name, state });
    }
    distros
}

/// The installed distributions in the order the lists show them: the
/// default, then the others by name.
fn listed_distros(registered: Vec<RegisteredDistro>, default_id: Option<&str>) -> Vec<WslDistro> {
    let mut distros: Vec<WslDistro> = registered
        .into_iter()
        .filter(|distro| distro.state.is_none_or(|state| state == STATE_INSTALLED))
        .filter(|distro| {
            let name = distro.name.trim();
            !name.is_empty()
                && !DOCKER_DISTROS
                    .iter()
                    .any(|docker| name.eq_ignore_ascii_case(docker))
        })
        .map(|distro| WslDistro {
            is_default: default_id.is_some_and(|id| id.eq_ignore_ascii_case(&distro.id)),
            name: distro.name.trim().to_string(),
        })
        .collect();
    distros.sort_by(|a, b| {
        b.is_default
            .cmp(&a.is_default)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    distros
}

/// The string value `name` of `subkey` under `key`, when it has one that fits.
fn read_string(
    key: HKEY,
    subkey: impl windows::core::Param<PCWSTR>,
    name: PCWSTR,
) -> Option<String> {
    let mut buffer = [0u16; 256];
    let mut size = std::mem::size_of_val(&buffer) as u32;
    // SAFETY: `buffer` holds `size` bytes, the most the call writes.
    let status = unsafe {
        RegGetValueW(
            key,
            subkey,
            name,
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    if status.is_err() {
        return None;
    }
    // The size counts the terminating null, which RegGetValueW always writes.
    let length = (size as usize / 2).saturating_sub(1);
    Some(String::from_utf16_lossy(&buffer[..length]))
}

/// The DWORD value `name` of `subkey` under `key`, when it has one.
fn read_dword(key: HKEY, subkey: impl windows::core::Param<PCWSTR>, name: PCWSTR) -> Option<u32> {
    let mut value = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: `value` and `size` outlive the call, which writes at most `size` bytes.
    let status = unsafe {
        RegGetValueW(
            key,
            subkey,
            name,
            RRF_RT_REG_DWORD,
            None,
            Some(std::ptr::from_mut(&mut value).cast()),
            Some(&mut size),
        )
    };
    status.is_ok().then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registered(id: &str, name: &str, state: Option<u32>) -> RegisteredDistro {
        RegisteredDistro {
            id: id.to_string(),
            name: name.to_string(),
            state,
        }
    }

    #[test]
    fn lists_the_default_first_then_by_name_without_docker_or_unfinished_ones() {
        let distros = listed_distros(
            vec![
                registered("{1}", "docker-desktop", Some(1)),
                registered("{2}", "debian", Some(1)),
                registered("{3}", "Ubuntu-24.04", Some(1)),
                registered("{4}", "Arch", None),
                registered("{5}", "Installing", Some(3)),
                registered("{6}", "Docker-Desktop-Data", Some(1)),
            ],
            Some("{3}"),
        );
        let names: Vec<_> = distros
            .iter()
            .map(|d| (d.name.as_str(), d.is_default))
            .collect();
        assert_eq!(
            names,
            [("Ubuntu-24.04", true), ("Arch", false), ("debian", false)]
        );
    }

    #[test]
    fn matches_the_default_guid_whatever_its_case() {
        let distros = listed_distros(
            vec![registered("{AB-cd}", "Ubuntu", Some(1))],
            Some("{ab-CD}"),
        );
        assert!(distros[0].is_default);
        assert!(!listed_distros(vec![registered("{1}", "Ubuntu", Some(1))], None)[0].is_default);
    }

    #[test]
    fn reads_the_installed_distributions() {
        // Whatever this machine has, names come out whole and at most one is the default.
        let distros = installed_distros();
        assert!(distros
            .iter()
            .all(|d| !d.name.is_empty() && !d.name.contains('\0')));
        assert!(distros.iter().filter(|d| d.is_default).count() <= 1);
    }
}
