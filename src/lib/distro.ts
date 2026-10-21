/**
 * What a host's `/etc/os-release` says about it. Linux and FreeBSD have the
 * file; the server monitor fills it in from `sw_vers` on macOS.
 */
export type LinuxDistributionInfo = {
  id: string
  idLike: string[]
  name: string
  prettyName: string
  versionId?: string
}

/**
 * The distributions with an icon of their own, generic Linux first, then
 * roughly by how often they run on servers, then the other systems the
 * server monitor reads. Each has `<id>.svg` in `public/assets/distro-icons`.
 */
export const DISTRO_IDS = [
  "linux",
  "ubuntu",
  "debian",
  "centos",
  "rhel",
  "fedora",
  "rocky",
  "alma",
  "oracle",
  "amazon",
  "suse",
  "alpine",
  "arch",
  "manjaro",
  "endeavouros",
  "cachyos",
  "garuda",
  "gentoo",
  "nixos",
  "void",
  "slackware",
  "kali",
  "parrot",
  "linuxmint",
  "popos",
  "elementary",
  "zorin",
  "deepin",
  "mx",
  "antix",
  "biglinux",
  "nobara",
  "bazzite",
  "freebsd",
  "macos",
] as const

export type DistroId = (typeof DISTRO_IDS)[number]

// Product names, which stay the same in every language.
const DISTRO_NAMES: Record<DistroId, string> = {
  linux: "Linux",
  ubuntu: "Ubuntu",
  debian: "Debian",
  centos: "CentOS",
  rhel: "Red Hat Enterprise Linux",
  fedora: "Fedora",
  rocky: "Rocky Linux",
  alma: "AlmaLinux",
  oracle: "Oracle Linux",
  amazon: "Amazon Linux",
  suse: "openSUSE",
  alpine: "Alpine Linux",
  arch: "Arch Linux",
  manjaro: "Manjaro",
  endeavouros: "EndeavourOS",
  cachyos: "CachyOS",
  garuda: "Garuda Linux",
  gentoo: "Gentoo",
  nixos: "NixOS",
  void: "Void Linux",
  slackware: "Slackware",
  kali: "Kali Linux",
  parrot: "Parrot OS",
  linuxmint: "Linux Mint",
  popos: "Pop!_OS",
  elementary: "elementary OS",
  zorin: "Zorin OS",
  deepin: "deepin",
  mx: "MX Linux",
  antix: "antiX",
  biglinux: "BigLinux",
  nobara: "Nobara",
  bazzite: "Bazzite",
  freebsd: "FreeBSD",
  macos: "macOS",
}

// Checked in order against each value, so names that contain another's
// ("linuxmint", "archlinux") come before it ("mint", "arch").
const DISTRO_RULES: Array<{ patterns: string[]; id: DistroId; exact?: boolean }> = [
  { patterns: ["archlinux"], id: "arch" },
  { patterns: ["ubuntu"], id: "ubuntu" },
  { patterns: ["linuxmint", "linux mint"], id: "linuxmint" },
  { patterns: ["mint"], id: "linuxmint" },
  { patterns: ["debian"], id: "debian" },
  { patterns: ["gentoo"], id: "gentoo" },
  { patterns: ["manjaro"], id: "manjaro" },
  { patterns: ["kali"], id: "kali" },
  { patterns: ["pop!_os", "pop_os"], id: "popos" },
  { patterns: ["pop os", "pop!"], id: "popos" },
  { patterns: ["zorin"], id: "zorin" },
  { patterns: ["elementary"], id: "elementary" },
  { patterns: ["mx linux"], id: "mx" },
  { patterns: ["mx"], id: "mx", exact: true },
  { patterns: ["endeavouros"], id: "endeavouros" },
  { patterns: ["endeavour"], id: "endeavouros" },
  { patterns: ["cachyos"], id: "cachyos" },
  { patterns: ["nixos", "nix os"], id: "nixos" },
  { patterns: ["nobara"], id: "nobara" },
  { patterns: ["bazzite"], id: "bazzite" },
  { patterns: ["antix"], id: "antix" },
  { patterns: ["biglinux", "big linux"], id: "biglinux" },
  { patterns: ["deepin"], id: "deepin" },
  { patterns: ["garuda"], id: "garuda" },
  { patterns: ["slackware"], id: "slackware" },
  { patterns: ["void"], id: "void" },
  { patterns: ["parrot"], id: "parrot" },
  { patterns: ["rocky"], id: "rocky" },
  { patterns: ["alma"], id: "alma" },
  { patterns: ["centos"], id: "centos" },
  { patterns: ["rhel", "redhat", "red hat"], id: "rhel" },
  { patterns: ["fedora"], id: "fedora" },
  { patterns: ["suse", "opensuse"], id: "suse" },
  { patterns: ["arch"], id: "arch" },
  { patterns: ["alpine"], id: "alpine" },
  { patterns: ["amazon"], id: "amazon" },
  { patterns: ["amzn"], id: "amazon", exact: true },
  { patterns: ["oracle"], id: "oracle" },
  { patterns: ["freebsd"], id: "freebsd" },
  { patterns: ["macos", "mac os x"], id: "macos" },
]

/**
 * The distribution whose icon a host shows; generic Linux for one without its
 * own. What the host says it is wins over what it says it is like, so Linux
 * Mint, which is like Ubuntu, shows its own.
 */
export function detectDistroId(info?: LinuxDistributionInfo): DistroId {
  const values = [info?.id, info?.name, info?.prettyName, ...(info?.idLike ?? [])]
    .filter((value): value is string => Boolean(value))
    .map((value) => value.toLowerCase())

  for (const value of values) {
    const rule = DISTRO_RULES.find(({ patterns, exact }) =>
      patterns.some((pattern) => (exact ? value === pattern : value.includes(pattern)))
    )
    if (rule) return rule.id
  }

  return "linux"
}

export function isDistroId(value: unknown): value is DistroId {
  return (DISTRO_IDS as readonly unknown[]).includes(value)
}

export function distroName(id: DistroId): string {
  return DISTRO_NAMES[id]
}

export function distroIconSrc(id: DistroId): string {
  return `/assets/distro-icons/${id}.svg`
}
