import { useEffect, useMemo, useState } from "react"
import { openUrl } from "@tauri-apps/plugin-opener"
import { ArrowLeft, ArrowUpRight, ChevronRight, Search } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { cn } from "@/lib/utils"

// Shape of public/licenses.json, written by scripts/gen-licenses.mjs.
type LicenseSource = "rust" | "npm" | "other"

interface LicenseFile {
  name: string
  // Index into LicenseData.texts.
  text: number
  // The standard SPDX text, used when the package ships no license file.
  spdx?: boolean
}

interface LicensePackage {
  name: string
  version: string
  license: string
  source: LicenseSource
  url: string
  files: LicenseFile[]
}

interface LicenseData {
  packages: LicensePackage[]
  texts: string[]
}

type SourceFilter = "all" | LicenseSource

let licenseDataRequest: Promise<LicenseData> | null = null

// Fetched on first use and kept, since the file is about a megabyte.
function loadLicenseData() {
  if (!licenseDataRequest) {
    licenseDataRequest = fetch("/licenses.json").then((response) => {
      if (!response.ok) {
        throw new Error(`licenses.json: ${response.status}`)
      }
      return response.json() as Promise<LicenseData>
    })
    licenseDataRequest.catch(() => {
      licenseDataRequest = null
    })
  }
  return licenseDataRequest
}

function packageKey(pkg: LicensePackage) {
  return `${pkg.source}:${pkg.name}@${pkg.version}`
}

function LicenseRow({
  pkg,
  texts,
  expanded,
  onToggle,
}: {
  pkg: LicensePackage
  texts: string[]
  expanded: boolean
  onToggle: () => void
}) {
  const { t } = useTranslation()
  const [fileIndex, setFileIndex] = useState(0)
  const file = pkg.files[fileIndex] ?? pkg.files[0]
  const fileLabel = (candidate: LicenseFile) =>
    candidate.spdx ? `${candidate.name} · ${t("about.standardText")}` : candidate.name

  return (
    <li>
      <button
        type="button"
        aria-expanded={expanded}
        onClick={onToggle}
        className="hover:bg-accent/60 focus-visible:bg-accent/60 flex w-full items-center gap-2 px-3 py-2 text-left outline-none"
      >
        <ChevronRight
          className={cn(
            "text-muted-foreground size-3.5 shrink-0 transition-transform",
            expanded && "rotate-90"
          )}
        />
        <span className="min-w-0 truncate text-sm font-medium">{pkg.name}</span>
        {pkg.version && (
          <span className="text-muted-foreground shrink-0 font-mono text-xs">{pkg.version}</span>
        )}
        <span className="text-muted-foreground ml-auto max-w-[45%] shrink-0 truncate pl-3 text-xs">
          {pkg.license}
        </span>
      </button>
      {expanded && (
        <div className="flex flex-col gap-2 px-3 pt-1 pb-3 pl-8">
          <div className="flex flex-wrap items-center gap-1">
            {pkg.files.length > 1 &&
              pkg.files.map((candidate, index) => (
                <Button
                  key={candidate.name}
                  type="button"
                  size="xs"
                  variant={index === fileIndex ? "secondary" : "ghost"}
                  onClick={() => setFileIndex(index)}
                >
                  {fileLabel(candidate)}
                </Button>
              ))}
            {pkg.files.length === 1 && file && (
              <span className="text-muted-foreground text-xs">{fileLabel(file)}</span>
            )}
            <Button
              type="button"
              size="xs"
              variant="ghost"
              className="text-muted-foreground ml-auto"
              title={pkg.url}
              onClick={() => void openUrl(pkg.url)}
            >
              {t("about.homepage")}
              <ArrowUpRight />
            </Button>
          </div>
          {file && (
            <pre className="bg-muted/50 max-h-72 overflow-auto rounded-md p-3 font-mono text-[11px] leading-relaxed whitespace-pre-wrap select-text">
              {texts[file.text]}
            </pre>
          )}
        </div>
      )}
    </li>
  )
}

export function LicensesView({ onBack }: { onBack: () => void }) {
  const { t } = useTranslation()
  const [data, setData] = useState<LicenseData | null>(null)
  const [failed, setFailed] = useState(false)
  const [query, setQuery] = useState("")
  const [source, setSource] = useState<SourceFilter>("all")
  const [expandedKey, setExpandedKey] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false
    loadLicenseData()
      .then((loaded) => {
        if (!cancelled) {
          setData(loaded)
        }
      })
      .catch(() => {
        if (!cancelled) {
          setFailed(true)
        }
      })
    return () => {
      cancelled = true
    }
  }, [])

  const counts = useMemo(() => {
    const result: Record<SourceFilter, number> = { all: 0, rust: 0, npm: 0, other: 0 }
    for (const pkg of data?.packages ?? []) {
      result.all += 1
      result[pkg.source] += 1
    }
    return result
  }, [data])

  const visiblePackages = useMemo(() => {
    const needle = query.trim().toLowerCase()
    return (data?.packages ?? []).filter(
      (pkg) =>
        (source === "all" || pkg.source === source) &&
        (!needle ||
          pkg.name.toLowerCase().includes(needle) ||
          pkg.license.toLowerCase().includes(needle))
    )
  }, [data, query, source])

  const filters: Array<{ value: SourceFilter; label: string }> = [
    { value: "all", label: t("about.filterAll") },
    { value: "rust", label: "Rust" },
    { value: "npm", label: "npm" },
    { value: "other", label: t("about.filterOther") },
  ]

  return (
    <DialogContent className="flex h-[min(calc(100vh-2rem),760px)] flex-col sm:max-w-2xl">
      <DialogHeader className="text-left">
        <div className="flex items-center gap-2 pr-6">
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            className="-ml-2"
            aria-label={t("about.back")}
            title={t("about.back")}
            onClick={onBack}
          >
            <ArrowLeft />
          </Button>
          <DialogTitle>{t("about.licenses")}</DialogTitle>
        </div>
        <DialogDescription>
          {data ? t("about.licensesDesc", { count: counts.all }) : " "}
        </DialogDescription>
      </DialogHeader>

      <div className="flex flex-col gap-2 sm:flex-row sm:items-center">
        <div className="relative flex-1">
          <Search className="text-muted-foreground pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2" />
          <Input
            autoFocus
            value={query}
            placeholder={t("about.licensesSearch")}
            className="pl-8"
            onChange={(event) => setQuery(event.target.value)}
          />
        </div>
        <div className="bg-muted flex shrink-0 rounded-md p-0.5" role="group">
          {filters.map((filter) => (
            <button
              key={filter.value}
              type="button"
              aria-pressed={source === filter.value}
              onClick={() => setSource(filter.value)}
              className={cn(
                "text-muted-foreground focus-visible:ring-ring/50 rounded-[5px] px-2.5 py-1 text-xs font-medium transition-colors outline-none focus-visible:ring-[3px]",
                source === filter.value && "bg-background text-foreground shadow-xs"
              )}
            >
              {filter.label}
              {data && <span className="ml-1 opacity-60">{counts[filter.value]}</span>}
            </button>
          ))}
        </div>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto rounded-md border">
        {failed ? (
          <p className="text-muted-foreground p-6 text-center text-sm">{t("about.loadFailed")}</p>
        ) : !data ? (
          <p className="text-muted-foreground p-6 text-center text-sm">{t("common.loading")}</p>
        ) : visiblePackages.length === 0 ? (
          <p className="text-muted-foreground p-6 text-center text-sm">{t("about.noResults")}</p>
        ) : (
          <ul className="divide-y">
            {visiblePackages.map((pkg) => {
              const key = packageKey(pkg)
              return (
                <LicenseRow
                  key={key}
                  pkg={pkg}
                  texts={data.texts}
                  expanded={expandedKey === key}
                  onToggle={() => setExpandedKey(expandedKey === key ? null : key)}
                />
              )
            })}
          </ul>
        )}
      </div>
    </DialogContent>
  )
}
