/** What a Linux host's `/etc/os-release` says about it. */
export type LinuxDistributionInfo = {
  id: string
  idLike: string[]
  name: string
  prettyName: string
  versionId?: string
}

const DISTRO_RULES: Array<{ patterns: string[]; id: string; exact?: boolean }> = [
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
]

/**
 * The distribution whose icon a host shows; generic Linux for one without its
 * own. What the host says it is wins over what it says it is like, so Linux
 * Mint, which is like Ubuntu, shows its own.
 */
export function detectDistroId(info?: LinuxDistributionInfo): string {
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

export function distroIconSrc(id: string): string {
  return `/assets/distro-icons/${id}.svg`
}
