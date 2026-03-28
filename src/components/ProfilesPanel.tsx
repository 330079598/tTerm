import React, { useEffect, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { useTranslation } from "react-i18next"
import { Server, Trash2, Play, Pencil, ChevronDown, ChevronRight } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Tab } from "@/types/tab"

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
  <div
    style={{
      display: "flex",
      alignItems: "center",
      gap: 6,
      padding: "6px 12px 6px 24px",
      cursor: "default",
    }}
    className="hover:bg-muted"
  >
    <Server size={13} style={{ flexShrink: 0, color: "hsl(var(--muted-foreground))" }} />
    <div style={{ flex: 1, minWidth: 0 }}>
      <div
        style={{
          fontSize: 13,
          fontWeight: 500,
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
        }}
      >
        {profile.name}
      </div>
      {profile.host && (
        <div
          style={{
            fontSize: 11,
            color: "hsl(var(--muted-foreground))",
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
          {profile.username ? `${profile.username}@` : ""}
          {profile.host}
          {profile.port && profile.port !== 22 ? `:${profile.port}` : ""}
        </div>
      )}
    </div>
    <button
      title={t("profiles.edit")}
      onClick={() => onEdit(profile)}
      style={{
        background: "none",
        border: "none",
        cursor: "pointer",
        padding: "2px 4px",
        color: "hsl(var(--muted-foreground))",
      }}
      className="hover:text-foreground"
    >
      <Pencil size={13} />
    </button>
    <button
      title={t("profiles.delete")}
      onClick={() => onDelete(profile.id)}
      style={{
        background: "none",
        border: "none",
        cursor: "pointer",
        padding: "2px 4px",
        color: "hsl(var(--muted-foreground))",
      }}
      className="hover:text-destructive"
    >
      <Trash2 size={13} />
    </button>
    <Button
      size="sm"
      variant="ghost"
      style={{ height: 24, padding: "0 8px", fontSize: 12 }}
      onClick={() => onConnect(profile)}
    >
      <Play size={12} style={{ marginRight: 3 }} />
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
    const connection: Omit<Tab, "id" | "isActive"> = {
      title: profile.name,
      type: profile.connection_type as Tab["type"],
      isModified: false,
      connection: {
        type: profile.connection_type as Tab["type"],
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
      <div style={{ padding: "12px 16px", fontSize: 13, color: "hsl(var(--muted-foreground))" }}>
        {t("profiles.empty")}
      </div>
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
    <div style={{ padding: "4px 0" }}>
      <div
        style={{
          padding: "4px 12px 8px",
          fontSize: 13,
          fontWeight: 600,
          color: "hsl(var(--foreground))",
        }}
      >
        {t("profiles.title")}
      </div>
      {Object.entries(groups).map(([group, items]) => (
        <div key={group}>
          <button
            onClick={() => toggleGroup(group)}
            style={{
              display: "flex",
              alignItems: "center",
              gap: 4,
              width: "100%",
              padding: "4px 12px",
              background: "none",
              border: "none",
              cursor: "pointer",
              fontSize: 12,
              fontWeight: 600,
              color: "hsl(var(--muted-foreground))",
              textTransform: "none",
              letterSpacing: "0.05em",
            }}
            className="hover:text-foreground"
          >
            {collapsed[group] ? <ChevronRight size={12} /> : <ChevronDown size={12} />}
            {group}
            <span style={{ marginLeft: 4, fontWeight: 400, fontSize: 11 }}>({items.length})</span>
          </button>
          {!collapsed[group] &&
            items.map((profile) => (
              <ProfileRow
                key={profile.id}
                profile={profile}
                onConnect={handleConnect}
                onEdit={onEdit}
                onDelete={handleDelete}
                t={t}
              />
            ))}
        </div>
      ))}
    </div>
  )
}
