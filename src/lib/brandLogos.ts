/**
 * The logos of shells and tools a tab can show, in the order the picker lays
 * them out: shells, then platforms, then languages and tools, then services.
 */
export const BRAND_LOGO_IDS = [
  "bash",
  "zsh",
  "nushell",
  "git",
  "github",
  "gitlab",
  "linux",
  "apple",
  "raspberry-pi",
  "docker",
  "kubernetes",
  "python",
  "nodejs",
  "go",
  "vim",
  "tmux",
  "redis",
  "postgresql",
  "mysql",
  "mongodb",
  "nginx",
  "ansible",
  "terraform",
  "google-cloud",
  "cloudflare",
] as const

export type BrandLogoId = (typeof BRAND_LOGO_IDS)[number]

const BRAND_LOGOS: Record<BrandLogoId, { name: string; color: string }> = {
  bash: { name: "Bash", color: "#4eaa25" },
  zsh: { name: "Zsh", color: "#f15a24" },
  nushell: { name: "Nushell", color: "#4e9a06" },
  git: { name: "Git", color: "#f03c2e" },
  // Black logos and Tux's yellow fade into one theme or the other, so these
  // take the text color.
  github: { name: "GitHub", color: "" },
  gitlab: { name: "GitLab", color: "#fc6d26" },
  linux: { name: "Linux", color: "" },
  apple: { name: "Apple", color: "" },
  "raspberry-pi": { name: "Raspberry Pi", color: "#a22846" },
  docker: { name: "Docker", color: "#2496ed" },
  kubernetes: { name: "Kubernetes", color: "#326ce5" },
  python: { name: "Python", color: "#3776ab" },
  nodejs: { name: "Node.js", color: "#5fa04e" },
  go: { name: "Go", color: "#00add8" },
  vim: { name: "Vim", color: "#019733" },
  tmux: { name: "tmux", color: "#1bb91f" },
  redis: { name: "Redis", color: "#ff4438" },
  postgresql: { name: "PostgreSQL", color: "#4169e1" },
  mysql: { name: "MySQL", color: "#4479a1" },
  mongodb: { name: "MongoDB", color: "#47a248" },
  nginx: { name: "NGINX", color: "#009639" },
  ansible: { name: "Ansible", color: "#ee0000" },
  terraform: { name: "Terraform", color: "#844fba" },
  "google-cloud": { name: "Google Cloud", color: "#4285f4" },
  cloudflare: { name: "Cloudflare", color: "#f38020" },
}

export function isBrandLogoId(value: unknown): value is BrandLogoId {
  return (BRAND_LOGO_IDS as readonly unknown[]).includes(value)
}

/** The logo's owner's name for it, which is not translated. */
export function brandLogoName(id: BrandLogoId): string {
  return BRAND_LOGOS[id].name
}

/** The color a logo takes when picked: its brand's, or "" for the text color. */
export function brandLogoColor(id: BrandLogoId): string {
  return BRAND_LOGOS[id].color
}
