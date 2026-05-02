import React, {useEffect, useMemo, useRef, useState} from "react"
import {invoke} from "@tauri-apps/api/core"
import {useTranslation} from "react-i18next"
import {ChevronRight, Pencil, PlugZap, Plus, Search, Server, Terminal, Trash2, X} from "lucide-react"

import {Badge} from "@/components/ui/badge"
import {Button} from "@/components/ui/button"
import {Input} from "@/components/ui/input"
import {ScrollArea} from "@/components/ui/scroll-area"
import {useConfirmDialog} from "@/components/ui/app-dialog"
import {cn} from "@/lib/utils"
import {Tab, type ConnectionType, type SavedProfile} from "@/types/tab"

interface ProfilesPanelProps {
    onConnect: (connection: Omit<Tab, "id" | "isActive">) => void
    onEdit: (profile: SavedProfile) => void
    refreshKey?: number
    onCreate?: () => void
    onClose?: () => void
    surface?: "panel" | "plain"
    className?: string
}

export type {SavedProfile}

const UNGROUPED_KEY = "__ungrouped__"

const connectionTypeIcons = {
    ssh: Server,
    terminal: Terminal,
} as const

const buildConnectionSubtitle = (
    profile: SavedProfile,
    t: (key: string, options?: Record<string, unknown>) => string
) => {
    if (profile.connection_type === "terminal") {
        return t("profiles.localTerminal")
    }

    if (!profile.host) {
        return t("profiles.connectionDetailsUnavailable")
    }

    return `${profile.username ? `${profile.username}@` : ""}${profile.host}${profile.port && profile.port !== 22 ? `:${profile.port}` : ""}`
}

const buildMetaItems = (
    profile: SavedProfile,
    t: (key: string, options?: Record<string, unknown>) => string
) => {
    if (profile.connection_type === "terminal") {
        return [t("profiles.localTerminal")]
    }

    const items = ["SSH"]

    if (profile.auth_method === "key") {
        items.push(t("profiles.authMethodKey"))
    } else if (profile.auth_method === "password") {
        items.push(t("profiles.authMethodPassword"))
    }

    if (profile.port && profile.port !== 22) {
        items.push(t("profiles.portDisplay", {port: profile.port}))
    }

    return items
}

const ProfileRow: React.FC<{
    profile: SavedProfile
    isActive: boolean
    onConnect: (p: SavedProfile) => void
    onEdit: (p: SavedProfile) => void
    onDelete: (id: string) => void
    onFocusRow: (id: string) => void
    rowRef: (node: HTMLDivElement | null) => void
    t: (key: string, options?: Record<string, unknown>) => string
}> = ({profile, isActive, onConnect, onEdit, onDelete, onFocusRow, rowRef, t}) => {
    const connectionType = profile.connection_type as ConnectionType
    const Icon = connectionTypeIcons[connectionType] ?? Server
    const subtitle = buildConnectionSubtitle(profile, t)
    const metaItems = buildMetaItems(profile, t)

    return (
        <div
            ref={rowRef}
            aria-selected={isActive}
            onClick={() => onFocusRow(profile.id)}
            onDoubleClick={() => onConnect(profile)}
            className={cn(
                "group focus-visible:ring-ring/50 relative flex w-full items-start gap-3 rounded-xl border px-3 py-3 text-left transition-colors outline-none focus-visible:ring-[3px]",
                isActive
                    ? "border-border bg-muted/55 shadow-[inset_0_0_0_1px_hsl(var(--border)/0.45)]"
                    : "border-border/60 hover:bg-muted/30"
            )}
        >
            {isActive && (
                <div className="bg-foreground/28 absolute inset-y-2 left-0 w-0.5 rounded-full"/>
            )}
            <div
                className="bg-muted text-muted-foreground flex size-9 shrink-0 items-center justify-center rounded-lg border">
                <Icon size={16}/>
            </div>

            <div className="min-w-0 flex-1">
                <div className="flex min-w-0 items-center gap-2">
                    <div className="truncate text-sm font-medium tracking-[-0.01em]">{profile.name}</div>
                    <Badge variant="outline" className="hidden shrink-0 sm:inline-flex">
                        {profile.connection_type === "terminal" ? t("profiles.localBadge") : "SSH"}
                    </Badge>
                </div>

                <div className="text-muted-foreground mt-1 truncate text-sm">{subtitle}</div>

                <div className="text-muted-foreground mt-2 flex flex-wrap items-center gap-x-2 gap-y-1 text-xs">
                    {metaItems.map((item, index) => (
                        <React.Fragment key={`${profile.id}-${item}`}>
                            {index > 0 && <span className="text-border">•</span>}
                            <span>{item}</span>
                        </React.Fragment>
                    ))}
                </div>
            </div>

            <div
                className={cn(
                    "flex shrink-0 items-center gap-1 self-center transition-opacity",
                    isActive
                        ? "opacity-100"
                        : "opacity-0 group-focus-within:opacity-100 group-hover:opacity-100"
                )}
            >
                <Button
                    title={t("profiles.connect")}
                    type="button"
                    variant="ghost"
                    size="icon-sm"
                    onClick={(event) => {
                        event.stopPropagation()
                        onConnect(profile)
                    }}
                >
                    <PlugZap size={13}/>
                </Button>
                <Button
                    title={t("profiles.edit")}
                    type="button"
                    variant="ghost"
                    size="icon-sm"
                    onClick={(event) => {
                        event.stopPropagation()
                        onEdit(profile)
                    }}
                >
                    <Pencil size={13}/>
                </Button>
                <Button
                    title={t("profiles.delete")}
                    type="button"
                    variant="ghost"
                    size="icon-sm"
                    className="hover:text-destructive"
                    onClick={(event) => {
                        event.stopPropagation()
                        onDelete(profile.id)
                    }}
                >
                    <Trash2 size={13}/>
                </Button>
                <ChevronRight size={16} className="text-muted-foreground ml-0.5"/>
            </div>
        </div>
    )
}

export const ProfilesPanel: React.FC<ProfilesPanelProps> = ({
                                                                onConnect,
                                                                onEdit,
                                                                refreshKey,
                                                                onCreate,
                                                                onClose,
                                                                surface = "panel",
                                                                className,
                                                            }) => {
    const {t} = useTranslation()
    const searchInputRef = useRef<HTMLInputElement | null>(null)
    const rowRefs = useRef<Record<string, HTMLDivElement | null>>({})
    const [profiles, setProfiles] = useState<SavedProfile[]>([])
    const [searchQuery, setSearchQuery] = useState("")
    const [selectedProfileId, setSelectedProfileId] = useState<string | null>(null)
    const {confirm, ConfirmDialog} = useConfirmDialog()

    useEffect(() => {
        invoke<SavedProfile[]>("list_profiles")
            .then((result) => {
                setProfiles(result)
            })
            .catch((error) => {
                console.error("Failed to load profiles:", error)
            })
    }, [refreshKey])

    useEffect(() => {
        if (surface !== "plain") {
            return
        }

        const frame = requestAnimationFrame(() => {
            searchInputRef.current?.focus()
        })

        return () => cancelAnimationFrame(frame)
    }, [surface])

    const handleDelete = async (id: string) => {
        const profile = profiles.find((item) => item.id === id)
        const deletePrompt = profile
            ? `${t("profiles.deleteConfirm")}\n\n${profile.name}`
            : t("profiles.deleteConfirm")

        const confirmed = await confirm({
            title: t("profiles.delete"),
            description: deletePrompt,
            confirmText: t("profiles.delete"),
            cancelText: t("common.cancel"),
            variant: "destructive",
        })

        if (!confirmed) return

        try {
            await invoke("delete_profile", {id})
            setProfiles((prev) => prev.filter((item) => item.id !== id))
        } catch (error) {
            console.error("Failed to delete profile:", error)
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
                profileId: profile.id,
                profileName: profile.name,
                host: profile.host,
                port: profile.port,
                username: profile.username,
                privateKeyPath: profile.auth_method === "key" ? profile.private_key_path : undefined,
                keepaliveIntervalSecs: profile.keepalive_interval_secs,
                keepaliveCountMax: profile.keepalive_count_max,
            },
        }

        onConnect(connection)
    }

    const groupedProfiles = useMemo(() => {
        const query = searchQuery.trim().toLowerCase()
        const filteredProfiles = profiles
            .filter((profile) => {
                if (!query) return true

                const searchTarget = [
                    profile.name,
                    profile.group,
                    profile.host,
                    profile.username,
                    profile.connection_type,
                ]
                    .filter(Boolean)
                    .join(" ")
                    .toLowerCase()

                return searchTarget.includes(query)
            })
            .sort((a, b) => a.name.localeCompare(b.name))

        const groups = new Map<string, SavedProfile[]>()

        for (const profile of filteredProfiles) {
            const key = profile.group === "" ? UNGROUPED_KEY : profile.group
            const existingItems = groups.get(key)
            if (existingItems) {
                existingItems.push(profile)
            } else {
                groups.set(key, [profile])
            }
        }

        return Array.from(groups.entries()).sort(([left], [right]) => {
            if (left === UNGROUPED_KEY) return 1
            if (right === UNGROUPED_KEY) return -1
            return left.localeCompare(right)
        })
    }, [profiles, searchQuery])

    const flatProfiles = useMemo(
        () => groupedProfiles.flatMap(([, items]) => items),
        [groupedProfiles]
    )

    const activeProfileId = useMemo(() => {
        if (flatProfiles.length === 0) {
            return null
        }

        if (selectedProfileId && flatProfiles.some((profile) => profile.id === selectedProfileId)) {
            return selectedProfileId
        }

        return flatProfiles[0].id
    }, [flatProfiles, selectedProfileId])

    useEffect(() => {
        if (!activeProfileId) {
            return
        }

        rowRefs.current[activeProfileId]?.scrollIntoView({block: "nearest"})
    }, [activeProfileId])

    const hasProfiles = profiles.length > 0
    const hasFilteredResults = groupedProfiles.length > 0

    return (
        <section
            className={cn(
                "flex h-full min-h-[360px] flex-col",
                surface === "panel" && "bg-card rounded-2xl border shadow-sm",
                className
            )}
        >
            <div className="border-border/80 flex items-start justify-between gap-4 border-b px-5 py-4">
                <div className="min-w-0 pr-6">
                    <h2 className="text-base font-semibold tracking-[-0.02em]">{t("profiles.title")}</h2>
                    <p className="text-muted-foreground mt-1 text-sm">{t("profiles.description")}</p>
                </div>

                <div className="flex shrink-0 items-center gap-2">
                    {onCreate && (
                        <Button type="button" size="sm" className="shrink-0" onClick={onCreate}>
                            <Plus size={14}/>
                            {t("profiles.newConnection")}
                        </Button>
                    )}
                    {onClose && (
                        <Button
                            type="button"
                            variant="ghost"
                            size="icon-sm"
                            onClick={onClose}
                            title={t("common.close")}
                        >
                            <X size={16}/>
                        </Button>
                    )}
                </div>
            </div>

            <div className="px-5 pt-4 pb-3">
                <div className="relative">
                    <Search
                        className="text-muted-foreground pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2"/>
                    <Input
                        ref={searchInputRef}
                        type="search"
                        value={searchQuery}
                        onChange={(event) => setSearchQuery(event.target.value)}
                        placeholder={t("profiles.searchPlaceholder")}
                        className="pl-9"
                    />
                </div>
            </div>

            <ScrollArea className="flex-1 px-3 pb-3">
                {!hasProfiles && (
                    <div
                        className="flex min-h-[260px] flex-col items-center justify-center rounded-xl border border-dashed px-6 text-center">
                        <div className="text-sm font-medium">{t("profiles.empty")}</div>
                        <p className="text-muted-foreground mt-2 max-w-sm text-sm">
                            {t("profiles.emptyDescription")}
                        </p>
                        {onCreate && (
                            <Button type="button" size="sm" className="mt-4" onClick={onCreate}>
                                <Plus size={14}/>
                                {t("profiles.newConnection")}
                            </Button>
                        )}
                    </div>
                )}

                {hasProfiles && !hasFilteredResults && (
                    <div
                        className="flex min-h-[220px] flex-col items-center justify-center rounded-xl border border-dashed px-6 text-center">
                        <div className="text-sm font-medium">{t("profiles.searchEmpty")}</div>
                        <p className="text-muted-foreground mt-2 max-w-sm text-sm">
                            {t("profiles.searchEmptyDescription")}
                        </p>
                    </div>
                )}

                {hasFilteredResults && (
                    <div className="space-y-5 px-2 pb-2">
                        {groupedProfiles.map(([group, items]) => (
                            <section key={group} className="space-y-2">
                                <div
                                    className="text-muted-foreground flex items-center justify-between px-1 text-[11px] font-semibold tracking-[0.08em]">
                  <span className="whitespace-pre-wrap">
                    {group === UNGROUPED_KEY ? t("profiles.ungrouped") : group}
                  </span>
                                    <span>{items.length}</span>
                                </div>

                                <div className="space-y-2">
                                    {items.map((profile) => (
                                        <ProfileRow
                                            key={profile.id}
                                            profile={profile}
                                            isActive={profile.id === activeProfileId}
                                            onConnect={handleConnect}
                                            onEdit={onEdit}
                                            onDelete={handleDelete}
                                            onFocusRow={setSelectedProfileId}
                                            rowRef={(node) => {
                                                rowRefs.current[profile.id] = node
                                            }}
                                            t={t}
                                        />
                                    ))}
                                </div>
                            </section>
                        ))}
                    </div>
                )}
            </ScrollArea>
            <ConfirmDialog/>
        </section>
    )
}
