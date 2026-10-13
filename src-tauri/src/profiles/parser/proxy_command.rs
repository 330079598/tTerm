//! Reads what an SSH config's `ProxyCommand` does. A command that only hands
//! the connection to an HTTP or SOCKS5 proxy (`nc -X`, `ncat --proxy`,
//! `connect`, `corkscrew`) becomes that proxy server, which tTerm dials
//! itself, and `ssh -W %h:%p` becomes a jump host, as `ProxyJump` does. Any
//! other command is kept to run as it is.

use super::words::split_ssh_words;
use crate::proxy::ProxyKind;

/// What a host's `ProxyCommand` turns into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ProxyCommandUse {
    /// `ProxyCommand none`.
    Direct,
    Server {
        kind: ProxyKind,
        host: String,
        port: u16,
    },
    /// A jump host, written as `ProxyJump` writes one: `[user@]host[:port]`.
    Jump(String),
    Command(String),
}

pub(super) fn classify(command: &str) -> ProxyCommandUse {
    let command = command.trim();
    if command.eq_ignore_ascii_case("none") {
        return ProxyCommandUse::Direct;
    }
    let words = split_ssh_words(command);
    let Some((_, args)) = words.split_first() else {
        return ProxyCommandUse::Command(command.to_string());
    };
    // The program as written: word splitting would read the backslashes of
    // a Windows path as escapes.
    let program = command.split_whitespace().next().unwrap_or_default();
    let recognized = match program_name(program.trim_matches(['"', '\''])).as_str() {
        "nc" | "netcat" => netcat(args),
        "ncat" => ncat(args),
        "connect" | "connect-proxy" => connect(args),
        "corkscrew" => corkscrew(args),
        "ssh" => {
            return ssh_jump(args).map_or_else(
                || ProxyCommandUse::Command(command.to_string()),
                ProxyCommandUse::Jump,
            )
        }
        _ => None,
    };
    match recognized {
        Some((kind, host, port)) => ProxyCommandUse::Server { kind, host, port },
        None => ProxyCommandUse::Command(command.to_string()),
    }
}

/// `/usr/bin/nc` and `C:\Git\mingw64\bin\connect.exe` as `nc` and `connect`.
fn program_name(program: &str) -> String {
    let name = program.rsplit(['/', '\\']).next().unwrap_or(program);
    let name = name.to_ascii_lowercase();
    name.strip_suffix(".exe")
        .map(str::to_string)
        .unwrap_or(name)
}

/// Whether `args` end with the target, `%h %p`, and nothing else follows.
fn ends_with_target(rest: &[&str]) -> bool {
    rest == ["%h", "%p"]
}

/// `host:port`, `[v6]:port` or a host alone with the protocol's usual port.
fn host_port(value: &str, default_port: u16) -> Option<(String, u16)> {
    let (host, port) = if let Some(bracketed) = value.strip_prefix('[') {
        let (host, rest) = bracketed.split_once(']')?;
        match rest.strip_prefix(':') {
            Some(port) => (host, port.parse().ok()?),
            None if rest.is_empty() => (host, default_port),
            None => return None,
        }
    } else {
        match value.split_once(':') {
            Some((host, port)) if !port.contains(':') => (host, port.parse().ok()?),
            // More than one colon: an IPv6 address without a port.
            Some(_) => (value, default_port),
            None => (value, default_port),
        }
    };
    (!host.is_empty() && port != 0).then(|| (host.to_string(), port))
}

type Server = (ProxyKind, String, u16);

/// OpenBSD netcat: `nc [-X connect|5] -x proxy[:port] %h %p`.
fn netcat(args: &[String]) -> Option<Server> {
    let (mut protocol, mut proxy, mut rest) = (None, None, Vec::new());
    let mut args = args.iter().map(String::as_str);
    while let Some(arg) = args.next() {
        match arg {
            "-X" => protocol = Some(args.next()?),
            "-x" => proxy = Some(args.next()?),
            _ if arg.starts_with("-X") => protocol = Some(&arg[2..]),
            _ if arg.starts_with("-x") => proxy = Some(&arg[2..]),
            _ => rest.push(arg),
        }
    }
    if !ends_with_target(&rest) {
        return None;
    }
    // Without -X, netcat speaks SOCKS5.
    let (kind, default_port) = match protocol {
        None | Some("5") => (ProxyKind::Socks5, 1080),
        Some(protocol) if protocol.eq_ignore_ascii_case("connect") => (ProxyKind::Http, 3128),
        Some(_) => return None,
    };
    let (host, port) = host_port(proxy?, default_port)?;
    Some((kind, host, port))
}

/// Nmap's ncat: `ncat --proxy proxy[:port] [--proxy-type http|socks5] %h %p`.
fn ncat(args: &[String]) -> Option<Server> {
    let (mut proxy_type, mut proxy, mut rest) = ("http", None, Vec::new());
    let mut args = args.iter().map(String::as_str);
    while let Some(arg) = args.next() {
        match arg {
            "--proxy" => proxy = Some(args.next()?),
            "--proxy-type" => proxy_type = args.next()?,
            _ => rest.push(arg),
        }
    }
    if !ends_with_target(&rest) {
        return None;
    }
    let (kind, default_port) = match proxy_type {
        "http" => (ProxyKind::Http, 3128),
        "socks5" => (ProxyKind::Socks5, 1080),
        _ => return None,
    };
    let (host, port) = host_port(proxy?, default_port)?;
    Some((kind, host, port))
}

/// connect-proxy: `connect -H proxy[:port] %h %p` or `connect -S proxy[:port] %h %p`.
fn connect(args: &[String]) -> Option<Server> {
    let (mut server, mut rest) = (None, Vec::new());
    let mut args = args.iter().map(String::as_str);
    while let Some(arg) = args.next() {
        match arg {
            "-H" => server = Some((ProxyKind::Http, 8080, args.next()?)),
            "-S" => server = Some((ProxyKind::Socks5, 1080, args.next()?)),
            // SOCKS5 is already connect's default.
            "-5" => {}
            _ => rest.push(arg),
        }
    }
    if !ends_with_target(&rest) {
        return None;
    }
    let (kind, default_port, proxy) = server?;
    let (host, port) = host_port(proxy, default_port)?;
    Some((kind, host, port))
}

/// `corkscrew proxy port %h %p`; a fifth argument, its password file, is not read.
fn corkscrew(args: &[String]) -> Option<Server> {
    let [host, port, target_host, target_port] = args else {
        return None;
    };
    if !ends_with_target(&[target_host.as_str(), target_port.as_str()]) {
        return None;
    }
    let port = port.parse().ok().filter(|port| *port != 0)?;
    Some((ProxyKind::Http, host.clone(), port))
}

/// `ssh [-q] [-p port] [-l user] -W %h:%p [user@]jump` as `[user@]jump[:port]`.
fn ssh_jump(args: &[String]) -> Option<String> {
    let (mut forward, mut port, mut user, mut destination) = (None, None, None, None);
    let mut args = args.iter().map(String::as_str);
    while let Some(arg) = args.next() {
        match arg {
            "-W" => forward = Some(args.next()?),
            "-p" => port = Some(args.next()?.parse::<u16>().ok()?),
            "-l" => user = Some(args.next()?),
            // Quiet, no TTY, no X11 or agent forwarding: none change the jump.
            "-q" | "-T" | "-x" | "-a" => {}
            _ if arg.starts_with('-') => return None,
            _ if destination.is_none() => destination = Some(arg),
            _ => return None,
        }
    }
    if !matches!(forward, Some("%h:%p" | "[%h]:%p")) {
        return None;
    }
    let destination = destination?;
    let (destination_user, host) = match destination.rsplit_once('@') {
        Some((user, host)) => (Some(user), host),
        None => (None, destination),
    };
    if host.is_empty() || host.contains('%') {
        return None;
    }
    let mut jump = String::new();
    if let Some(user) = destination_user.or(user) {
        jump.push_str(user);
        jump.push('@');
    }
    match port {
        Some(port) if host.contains(':') => jump.push_str(&format!("[{host}]:{port}")),
        Some(port) => jump.push_str(&format!("{host}:{port}")),
        None => jump.push_str(host),
    }
    Some(jump)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(kind: ProxyKind, host: &str, port: u16) -> ProxyCommandUse {
        ProxyCommandUse::Server {
            kind,
            host: host.to_string(),
            port,
        }
    }

    #[test]
    fn proxy_server_commands_become_servers() {
        use ProxyKind::{Http, Socks5};
        let cases = [
            (
                "nc -X 5 -x 127.0.0.1:1080 %h %p",
                server(Socks5, "127.0.0.1", 1080),
            ),
            (
                "/usr/bin/nc -X connect -x proxy.corp:8080 %h %p",
                server(Http, "proxy.corp", 8080),
            ),
            (
                "nc -x127.0.0.1:7890 %h %p",
                server(Socks5, "127.0.0.1", 7890),
            ),
            (
                "nc -X connect -x proxy.corp %h %p",
                server(Http, "proxy.corp", 3128),
            ),
            ("ncat --proxy [::1]:3128 %h %p", server(Http, "::1", 3128)),
            (
                "ncat --proxy 10.0.0.1 --proxy-type socks5 %h %p",
                server(Socks5, "10.0.0.1", 1080),
            ),
            ("connect -H proxy:3128 %h %p", server(Http, "proxy", 3128)),
            (
                r"C:\Git\mingw64\bin\connect.exe -S 127.0.0.1:1080 %h %p",
                server(Socks5, "127.0.0.1", 1080),
            ),
            (
                "corkscrew proxy.corp 8080 %h %p",
                server(Http, "proxy.corp", 8080),
            ),
        ];
        for (command, expected) in cases {
            assert_eq!(classify(command), expected, "{command}");
        }
    }

    #[test]
    fn ssh_w_becomes_a_jump_host() {
        let cases = [
            ("ssh -W %h:%p bastion", "bastion"),
            ("ssh -q -W %h:%p ops@bastion", "ops@bastion"),
            ("ssh bastion -p 2222 -l ops -W [%h]:%p", "ops@bastion:2222"),
            ("ssh -p 22 -W %h:%p fe80::1", "[fe80::1]:22"),
        ];
        for (command, jump) in cases {
            assert_eq!(
                classify(command),
                ProxyCommandUse::Jump(jump.to_string()),
                "{command}"
            );
        }
    }

    #[test]
    fn other_commands_are_kept_as_they_are() {
        for command in [
            "nc -X 4 -x proxy:1080 %h %p",
            "nc -X connect -x proxy:8080 -P me %h %p",
            "ncat --proxy proxy:3128 --proxy-auth me:secret %h %p",
            "connect %h %p",
            "corkscrew proxy 8080 %h %p ~/.corkscrew-auth",
            "ssh -i ~/.ssh/jump -W %h:%p bastion",
            "ssh bastion nc %h %p",
            "cloudflared access ssh --hostname %h",
            "sh -c 'nc %h %p'",
        ] {
            assert_eq!(
                classify(command),
                ProxyCommandUse::Command(command.to_string()),
                "{command}"
            );
        }
        assert_eq!(classify(" none "), ProxyCommandUse::Direct);
    }
}
