import React, { useEffect, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { useTranslation } from "react-i18next"
import { Server, Trash2, Play, Pencil, ChevronDown, ChevronRight } from "lucide-react"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Tab, type ConnectionType } from "@/types/tab"

interface SavedProfile {
  id: string
  name: string
  group: string
  connection_type: string
  host?: string
  port?: number
  username?: string
  auth_method?: string
  private_key_path?: string
  reconnect: boolean
  reconnect_delay_secs: number
  reconnect_max_delay_secs: number
  reconnect_max_retries: number
  keepalive_interval_secs: number
  keepalive_count_max: number
}

interface ProfilesPanelProps {
  onConnect: (connection: Omit<Tab, "id" | "isActive">) => void
  onEdit: (profile: SavedProfile) => void
  refreshKey?: number
}

export type { SavedProfile }

const ProfileRow: React.FC<{
  profile: SavedProfile
  onConnect: (p: SavedProfile) => void
  onEdit: (p: SavedProfile) => void
  onDelete: (id: string) => void
  t: (key: string) => string
}> = ({ profile, onConnect, onEdit, onDelete, t }) => (
  <div className="hover:bg-muted/50 flex items-center gap-3 rounded-md px-3 py-2 transition-colors">
    <div className="bg-secondary text-muted-foreground flex size-8 shrink-0 items-center justify-center rounded-md">
      <Server size={14} />
    </div>
    <div className="min-w-0 flex-1">
      <div className="truncate text-sm font-medium">{profile.name}</div>
      {profile.host && (
        <div className="text-muted-foreground truncate text-xs">
          {profile.username ? `${profile.username}@` : ""}
          {profile.host}
          {profile.port && profile.port !== 22 ? `:${profile.port}` : ""}
        </div>
      )}
    </div>
    <Button
      title={t("profiles.edit")}
      type="button"
      variant="ghost"
      size="icon-xs"
      onClick={() => onEdit(profile)}
    >
      <Pencil size={13} />
    </Button>
    <Button
      title={t("profiles.delete")}
      type="button"
      variant="ghost"
      size="icon-xs"
      className="hover:text-destructive"
      onClick={() => onDelete(profile.id)}
    >
      <Trash2 size={13} />
    </Button>
    <Button size="sm" variant="outline" onClick={() => onConnect(profile)}>
      <Play size={12} />
      {t("profiles.connect")}
    </Button>
  </div>
)

export const ProfilesPanel: React.FC<ProfilesPanelProps> = ({ onConnect, onEdit, refreshKey }) => {
  const { t } = useTranslation()
  const [profiles, setProfiles] = useState<SavedProfile[]>([])
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>({})

  useEffect(() => {
    invoke<SavedProfile[]>("list_profiles")
      .then((result) => {
        setProfiles(result)
      })
      .catch((e) => {
        console.error("Failed to load profiles:", e)
      })
  }, [refreshKey])

  const handleDelete = async (id: string) => {
    if (!confirm(t("profiles.deleteConfirm"))) return
    try {
      await invoke("delete_profile", { id })
      setProfiles((prev) => prev.filter((p) => p.id !== id))
    } catch (e) {
      console.error("Failed to delete profile:", e)
    }
  }

  const handleConnect = (profile: SavedProfile) => {
    const connectionType = profile.connection_type as ConnectionType
    const connection: Omit<Tab, "id" | "isActive"> = {
      title: profile.name,
      type: connectionType,
      isModified: false,
      connection: {
        type: connectionType,
        profileName: profile.name,
        host: profile.host,
        port: profile.port,
        username: profile.username,
        privateKeyPath: profile.auth_method === "key" ? profile.private_key_path : undefined,
        reconnect: profile.reconnect,
        reconnectDelaySecs: profile.reconnect_delay_secs,
        reconnectMaxDelaySecs: profile.reconnect_max_delay_secs,
        reconnectMaxRetries: profile.reconnect_max_retries,
        keepaliveIntervalSecs: profile.keepalive_interval_secs,
        keepaliveCountMax: profile.keepalive_count_max,
      },
    }
    onConnect(connection)
  }

  if (profiles.length === 0) {
    return (
      <Card style={{ minWidth: "min(380px, 90vw)", minHeight: "min(300px, 40vh)" }}>
        <CardContent className="text-muted-foreground p-4 text-sm">
          {t("profiles.empty")}
        </CardContent>
      </Card>
    )
  }

  // Group profiles
  const groups: Record<string, SavedProfile[]> = {}
  for (const p of profiles) {
    const key = p.group || t("profiles.ungrouped")
    if (!groups[key]) groups[key] = []
    groups[key].push(p)
  }

  const toggleGroup = (g: string) => setCollapsed((prev) => ({ ...prev, [g]: !prev[g] }))

  return (
    <Card style={{ minWidth: "min(380px, 90vw)", minHeight: "min(300px, 40vh)" }}>
      <CardHeader className="pb-3">
        <CardTitle className="text-sm">{t("profiles.title")}</CardTitle>
      </CardHeader>
      <CardContent className="space-y-3 p-2 pt-0">
        {Object.entries(groups).map(([group, items]) => (
          <div key={group} className="rounded-lg border">
            <Button
              type="button"
              variant="ghost"
              onClick={() => toggleGroup(group)}
              className="text-muted-foreground hover:text-foreground h-auto w-full justify-start rounded-b-none px-3 py-2 text-xs font-semibold tracking-[0.08em]"
            >
              {collapsed[group] ? <ChevronRight size={12} /> : <ChevronDown size={12} />}
              {group}
              <Badge variant="outline" className="ml-2 text-[10px] font-normal">
                {items.length}
              </Badge>
            </Button>
            {!collapsed[group] &&
              items.map((profile) => {
                return (
                  <ProfileRow
                    key={profile.id}
                    profile={profile}
                    onConnect={handleConnect}
                    onEdit={onEdit}
                    onDelete={handleDelete}
                    t={t}
                  />
                )
              })}
          </div>
        ))}
      </CardContent>
    </Card>
  )
}
