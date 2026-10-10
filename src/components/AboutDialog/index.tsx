import { useEffect, useState } from "react"
import { getVersion } from "@tauri-apps/api/app"
import { openUrl } from "@tauri-apps/plugin-opener"
import { ArrowUpRight, ChevronRight } from "lucide-react"
import { useTranslation } from "react-i18next"

import appIcon from "/app-icon.svg"
import { LicensesView } from "@/components/AboutDialog/LicensesView"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"

const REPOSITORY_URL = "https://github.com/330079598/tTerm"
const fallbackAppVersion = import.meta.env.PACKAGE_VERSION ?? "0.0.0"

// The projects tTerm leans on most; every shipped package, these included, is
// listed with its license text under Open Source Licenses.
const ACKNOWLEDGEMENTS = [
  { name: "Tauri", key: "tauri", url: "https://tauri.app" },
  { name: "xterm.js", key: "xterm", url: "https://xtermjs.org" },
  { name: "russh", key: "russh", url: "https://github.com/Eugeny/russh" },
  { name: "React", key: "react", url: "https://react.dev" },
  { name: "CodeMirror", key: "codemirror", url: "https://codemirror.net" },
  { name: "Dockview", key: "dockview", url: "https://dockview.dev" },
  {
    name: "portable-pty",
    key: "portablePty",
    url: "https://github.com/wezterm/wezterm/tree/main/pty",
  },
  { name: "Tokio", key: "tokio", url: "https://tokio.rs" },
  { name: "rustls · ring", key: "rustls", url: "https://github.com/rustls/rustls" },
  { name: "SQLite", key: "sqlite", url: "https://sqlite.org" },
  { name: "Tailwind CSS", key: "tailwind", url: "https://tailwindcss.com" },
  { name: "Lucide", key: "lucide", url: "https://lucide.dev" },
  {
    name: "Material Icon Theme",
    key: "materialIcons",
    url: "https://github.com/material-extensions/vscode-material-icon-theme",
  },
  {
    name: "iTerm2-Color-Schemes",
    key: "colorSchemes",
    url: "https://github.com/mbadolato/iTerm2-Color-Schemes",
  },
] as const

function GitHubIcon() {
  return (
    <svg role="img" viewBox="0 0 24 24" className="size-4 fill-current" aria-hidden="true">
      <path d="M12 .297c-6.63 0-12 5.373-12 12 0 5.303 3.438 9.8 8.205 11.385.6.113.82-.258.82-.577 0-.285-.01-1.04-.015-2.04-3.338.724-4.042-1.61-4.042-1.61C4.422 18.07 3.633 17.7 3.633 17.7c-1.087-.744.084-.729.084-.729 1.205.084 1.838 1.236 1.838 1.236 1.07 1.835 2.809 1.305 3.495.998.108-.776.417-1.305.76-1.605-2.665-.3-5.466-1.332-5.466-5.93 0-1.31.465-2.38 1.235-3.22-.135-.303-.54-1.523.105-3.176 0 0 1.005-.322 3.3 1.23.96-.267 1.98-.399 3-.405 1.02.006 2.04.138 3 .405 2.28-1.552 3.285-1.23 3.285-1.23.645 1.653.24 2.873.12 3.176.765.84 1.23 1.91 1.23 3.22 0 4.61-2.805 5.625-5.475 5.92.42.36.81 1.096.81 2.22 0 1.606-.015 2.896-.015 3.286 0 .315.21.69.825.57C20.565 22.092 24 17.592 24 12.297c0-6.627-5.373-12-12-12" />
    </svg>
  )
}

function AboutView({ onShowLicenses }: { onShowLicenses: () => void }) {
  const { t } = useTranslation()
  const [version, setVersion] = useState(fallbackAppVersion)

  useEffect(() => {
    let cancelled = false
    getVersion()
      .then((appVersion) => {
        if (!cancelled) {
          setVersion(appVersion)
        }
      })
      .catch(() => {
        // Keep the build-time version when the Tauri app API is unavailable.
      })
    return () => {
      cancelled = true
    }
  }, [])

  return (
    <DialogContent className="flex max-h-[calc(100vh-2rem)] flex-col sm:max-w-xl">
      <DialogHeader className="flex-row items-center gap-4 text-left">
        <img src={appIcon} alt="" className="size-14 shrink-0 rounded-xl" draggable={false} />
        <div className="flex min-w-0 flex-col gap-1">
          <DialogTitle>{t("app.title")}</DialogTitle>
          <DialogDescription>{t("app.subtitle")}</DialogDescription>
          <p className="text-muted-foreground font-mono text-xs select-text">
            {t("app.version", { version })}
          </p>
        </div>
      </DialogHeader>

      <section className="flex min-h-0 flex-col gap-2 border-t pt-4">
        <div>
          <h3 className="text-sm font-medium">{t("about.acknowledgements")}</h3>
          <p className="text-muted-foreground mt-1 text-xs">{t("about.acknowledgementsDesc")}</p>
        </div>
        <ul className="-mx-1 grid min-h-0 grid-cols-2 gap-1 overflow-y-auto px-1 py-0.5">
          {ACKNOWLEDGEMENTS.map((project) => (
            <li key={project.key}>
              <button
                type="button"
                title={project.url}
                onClick={() => void openUrl(project.url)}
                className="group hover:bg-accent focus-visible:ring-ring/50 flex w-full flex-col items-start rounded-md px-2.5 py-1.5 text-left transition-colors outline-none focus-visible:ring-[3px]"
              >
                <span className="flex w-full items-center gap-1 text-sm font-medium">
                  <span className="truncate">{project.name}</span>
                  <ArrowUpRight className="text-muted-foreground size-3.5 shrink-0 opacity-0 transition-opacity group-hover:opacity-100 group-focus-visible:opacity-100" />
                </span>
                <span className="text-muted-foreground w-full truncate text-xs">
                  {t(`about.projects.${project.key}`)}
                </span>
              </button>
            </li>
          ))}
        </ul>
      </section>

      <DialogFooter className="border-t pt-4 sm:justify-between">
        <Button type="button" variant="ghost" onClick={() => void openUrl(REPOSITORY_URL)}>
          <GitHubIcon />
          GitHub
        </Button>
        <Button type="button" variant="outline" onClick={onShowLicenses}>
          {t("about.licenses")}
          <ChevronRight />
        </Button>
      </DialogFooter>
    </DialogContent>
  )
}

function AboutContent() {
  const [view, setView] = useState<"about" | "licenses">("about")

  return view === "about" ? (
    <AboutView onShowLicenses={() => setView("licenses")} />
  ) : (
    <LicensesView onBack={() => setView("about")} />
  )
}

export function AboutDialog({
  open,
  onOpenChange,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
}) {
  // Mounted only while open so the dialog starts on the About view each time.
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      {open && <AboutContent />}
    </Dialog>
  )
}
