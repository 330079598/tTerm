import React, { useCallback, useEffect, useMemo, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { Search, Trash2 } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { Input } from "@/components/ui/input"

import type { useConfirmDialog } from "@/components/ui/app-dialog"
import { useToast } from "@/hooks/use-toast"
import { onSyncApplied } from "@/lib/sync"
import { toErrorMessage } from "@/lib/utils"

export interface KnownHostEntry {
  id: number
  /** Missing for jump hosts, which are trusted per endpoint rather than per profile. */
  profileName?: string | null
  isJumpHost: boolean
  host: string
  port: number
  algorithm: string
  fingerprint: string
  trustedAt: number
}

/** Shown without a search box up to this many entries. */
const SEARCH_THRESHOLD = 6

export function filterKnownHosts(entries: KnownHostEntry[], query: string): KnownHostEntry[] {
  const normalized = query.trim().toLocaleLowerCase()
  if (!normalized) return entries
  return entries.filter((entry) =>
    [`${entry.host}:${entry.port}`, entry.profileName ?? "", entry.algorithm, entry.fingerprint]
      .join("\n")
      .toLocaleLowerCase()
      .includes(normalized)
  )
}

interface KnownHostsCardProps {
  confirm: ReturnType<typeof useConfirmDialog>["confirm"]
}

export const KnownHostsCard: React.FC<KnownHostsCardProps> = ({ confirm }) => {
  const { t } = useTranslation()
  const { toast } = useToast()
  const [entries, setEntries] = useState<KnownHostEntry[]>([])
  const [loadError, setLoadError] = useState<string | null>(null)
  const [query, setQuery] = useState("")
  const [deletingId, setDeletingId] = useState<number | null>(null)

  const reload = useCallback(async () => {
    try {
      setEntries(await invoke<KnownHostEntry[]>("list_known_hosts"))
      setLoadError(null)
    } catch (error) {
      setLoadError(toErrorMessage(error))
    }
  }, [])

  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect -- state is only set after await; false positive fixed upstream in facebook/react#36734
    void reload()
    return onSyncApplied(["knownHosts", "profiles"], () => void reload())
  }, [reload])

  const visibleEntries = useMemo(() => filterKnownHosts(entries, query), [entries, query])
  const dateFormat = useMemo(
    () => new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }),
    []
  )

  const handleDelete = async (entry: KnownHostEntry) => {
    const endpoint = `${entry.host}:${entry.port}`
    const confirmed = await confirm({
      title: t("knownHosts.deleteTitle"),
      description: t("knownHosts.deleteConfirm", { host: endpoint }),
      confirmText: t("common.delete"),
      cancelText: t("common.cancel"),
      variant: "destructive",
    })
    if (!confirmed) return

    setDeletingId(entry.id)
    try {
      await invoke<number>("delete_known_hosts", { ids: [entry.id] })
      setEntries((current) => current.filter((candidate) => candidate.id !== entry.id))
      toast({
        title: t("knownHosts.deleted"),
        description: t("knownHosts.deletedDesc", { host: endpoint }),
      })
    } catch (error) {
      toast({
        title: t("knownHosts.deleteFailed"),
        description: toErrorMessage(error),
        variant: "destructive",
      })
    } finally {
      setDeletingId(null)
    }
  }

  return (
    <Card>
      <CardContent className="space-y-3 p-4">
        <div className="flex items-center justify-between gap-3">
          <div>
            <div className="text-sm font-medium">{t("knownHosts.title")}</div>
            <div className="text-muted-foreground mt-1 text-xs leading-5">
              {t("knownHosts.description")}
            </div>
          </div>
          <Badge variant="secondary" className="shrink-0">
            {t("knownHosts.count", { count: entries.length })}
          </Badge>
        </div>

        {entries.length > SEARCH_THRESHOLD && (
          <div className="relative">
            <Search className="text-muted-foreground pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2" />
            <Input
              type="search"
              value={query}
              placeholder={t("knownHosts.searchPlaceholder")}
              aria-label={t("knownHosts.searchPlaceholder")}
              className="pl-9"
              onChange={(event) => setQuery(event.target.value)}
            />
          </div>
        )}

        {loadError ? (
          <p className="text-destructive text-xs leading-5" role="alert">
            {t("knownHosts.loadFailed", { error: loadError })}
          </p>
        ) : entries.length === 0 ? (
          <p className="text-muted-foreground text-xs leading-5">{t("knownHosts.empty")}</p>
        ) : visibleEntries.length === 0 ? (
          <p className="text-muted-foreground text-xs leading-5">{t("knownHosts.noMatches")}</p>
        ) : (
          <div className="border-border divide-border max-h-80 divide-y overflow-y-auto rounded-md border">
            {visibleEntries.map((entry) => (
              <div key={entry.id} className="flex items-start justify-between gap-3 px-3 py-2">
                <div className="min-w-0">
                  <div className="flex min-w-0 items-center gap-2">
                    <span className="truncate font-mono text-sm">
                      {entry.host}:{entry.port}
                    </span>
                    {entry.isJumpHost && (
                      <Badge variant="outline" className="h-5 shrink-0 px-1.5 text-[10px]">
                        {t("knownHosts.jumpHost")}
                      </Badge>
                    )}
                  </div>
                  <div className="text-muted-foreground truncate text-xs">
                    {[
                      entry.profileName,
                      entry.algorithm,
                      t("knownHosts.trustedAt", { date: dateFormat.format(entry.trustedAt) }),
                    ]
                      .filter(Boolean)
                      .join(" · ")}
                  </div>
                  <div className="text-muted-foreground mt-0.5 font-mono text-xs break-all">
                    {entry.fingerprint}
                  </div>
                </div>
                <Button
                  variant="ghost"
                  size="icon-sm"
                  className="text-destructive hover:text-destructive shrink-0"
                  disabled={deletingId !== null}
                  onClick={() => void handleDelete(entry)}
                  title={t("knownHosts.delete")}
                  aria-label={t("knownHosts.deleteLabel", { host: `${entry.host}:${entry.port}` })}
                >
                  <Trash2 size={14} />
                </Button>
              </div>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  )
}
