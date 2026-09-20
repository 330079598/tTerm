import React, { useMemo, useState } from "react"
import { AlertTriangle, ArrowRightLeft, Globe, Loader2, Undo2 } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Select } from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"
import { cn } from "@/lib/utils"
import type { SavedProfile } from "@/types/tab"
import type { TunnelKind, TunnelRule } from "@/types/tunnel"
import { TunnelRoute } from "@/components/TunnelsPanel/TunnelRoute"
import {
  buildSshCommand,
  createEmptyTunnel,
  parsePortInput,
  profileLabel,
  suggestTunnelName,
  TUNNEL_KINDS,
  validateTunnel,
  type TunnelFormError,
} from "@/components/TunnelsPanel/tunnelUtils"

const KIND_ICONS: Record<TunnelKind, React.ComponentType<{ className?: string }>> = {
  local: ArrowRightLeft,
  remote: Undo2,
  dynamic: Globe,
}

const LOOPBACK_HOSTS = new Set(["127.0.0.1", "localhost", "::1", "[::1]"])

interface TunnelDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** The rule being edited, or null to create a new one. */
  tunnel: TunnelRule | null
  profiles: SavedProfile[]
  onSave: (rule: TunnelRule) => Promise<void>
}

type TunnelFormProps = Omit<TunnelDialogProps, "open">

const TunnelForm: React.FC<TunnelFormProps> = ({ onOpenChange, tunnel, profiles, onSave }) => {
  const { t } = useTranslation()
  const [draft, setDraft] = useState<TunnelRule>(() => tunnel ?? createEmptyTunnel("local"))
  const [bindPortText, setBindPortText] = useState(() =>
    draft.bindPort ? String(draft.bindPort) : ""
  )
  const [destPortText, setDestPortText] = useState(() =>
    draft.destPort ? String(draft.destPort) : ""
  )
  // Until the user types a name, one is suggested from the host and destination.
  const [nameOverride, setNameOverride] = useState<string | null>(tunnel ? tunnel.name : null)
  const [error, setError] = useState<TunnelFormError | null>(null)
  const [saveError, setSaveError] = useState<string | null>(null)
  const [saving, setSaving] = useState(false)

  const profile = profiles.find((candidate) => candidate.id === draft.profileId)
  const hostLabel = profileLabel(profile)
  const bindPort = parsePortInput(bindPortText)
  const destPort = parsePortInput(destPortText)
  const effective: TunnelRule = {
    ...draft,
    bindPort,
    destPort,
    name: nameOverride ?? suggestTunnelName({ ...draft, bindPort, destPort }, hostLabel),
  }

  const exposesBindAddress =
    draft.kind !== "remote" &&
    draft.bindHost.trim() !== "" &&
    !LOOPBACK_HOSTS.has(draft.bindHost.trim())

  const sshCommand = useMemo(
    () => buildSshCommand(effective, profile),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [draft.kind, draft.bindHost, draft.destHost, bindPort, destPort, profile]
  )

  const update = (patch: Partial<TunnelRule>) => {
    setDraft((current) => ({ ...current, ...patch }))
    setError(null)
  }

  const selectKind = (kind: TunnelKind) => {
    if (kind === draft.kind) return
    if (kind === "dynamic" && !bindPortText) setBindPortText("1080")
    update({ kind })
  }

  const handleSave = async () => {
    const problem = validateTunnel(effective)
    if (problem) {
      setError(problem)
      return
    }
    setSaving(true)
    setSaveError(null)
    try {
      await onSave(effective)
      onOpenChange(false)
    } catch (err) {
      setSaveError(err instanceof Error ? err.message : String(err))
      setSaving(false)
    }
  }

  const fieldError = (...keys: TunnelFormError[]) =>
    error && keys.includes(error) ? (
      <p className="text-destructive mt-1 text-xs">
        {t(`tunnels.errors.${error}`, { defaultValue: "Check this field" })}
      </p>
    ) : null

  const bindLabel =
    draft.kind === "remote"
      ? t("tunnels.form.remoteBind", { defaultValue: "Listen on the server" })
      : t("tunnels.form.localBind", { defaultValue: "Listen on this device" })
  const destLabel =
    draft.kind === "remote"
      ? t("tunnels.form.remoteDest", { defaultValue: "Forward to (from this device)" })
      : t("tunnels.form.localDest", { defaultValue: "Forward to (from the SSH host)" })

  return (
    <>
      <DialogHeader className="border-border/80 border-b px-6 py-5">
        <DialogTitle>
          {tunnel
            ? t("tunnels.editTitle", { defaultValue: "Edit tunnel" })
            : t("tunnels.newTitle", { defaultValue: "New tunnel" })}
        </DialogTitle>
        <DialogDescription>
          {t("tunnels.dialogDescription", {
            defaultValue: "Forward a port through an SSH host you have already saved.",
          })}
        </DialogDescription>
      </DialogHeader>

      <div className="max-h-[68vh] space-y-5 overflow-y-auto px-6 py-5">
        <div
          role="radiogroup"
          aria-label={t("tunnels.form.type", { defaultValue: "Tunnel type" })}
          className="grid grid-cols-3 gap-2"
        >
          {TUNNEL_KINDS.map((kind) => {
            const Icon = KIND_ICONS[kind]
            const selected = draft.kind === kind
            return (
              <button
                key={kind}
                type="button"
                role="radio"
                aria-checked={selected}
                onClick={() => selectKind(kind)}
                className={cn(
                  "hover:bg-accent/50 focus-visible:ring-ring/50 flex flex-col items-start gap-1 rounded-lg border p-3 text-left transition-colors outline-none focus-visible:ring-[3px]",
                  selected && "border-primary bg-accent/40 ring-primary/30 ring-1"
                )}
              >
                <Icon
                  className={cn("size-4", selected ? "text-primary" : "text-muted-foreground")}
                />
                <span className="text-sm font-medium">
                  {t(`tunnels.kinds.${kind}.title`, { defaultValue: kind })}
                </span>
                <span className="text-muted-foreground text-xs leading-snug">
                  {t(`tunnels.kinds.${kind}.description`, { defaultValue: "" })}
                </span>
              </button>
            )
          })}
        </div>

        <div className="bg-muted/40 rounded-lg border p-3">
          <TunnelRoute
            kind={draft.kind}
            bindHost={draft.bindHost}
            bindPort={bindPort}
            destHost={draft.destHost}
            destPort={destPort}
            hostLabel={hostLabel || t("tunnels.route.server", { defaultValue: "SSH host" })}
          />
        </div>

        <div className="grid gap-4 sm:grid-cols-2">
          <div className="sm:col-span-2">
            <Label htmlFor="tunnel-host">
              {t("tunnels.form.host", { defaultValue: "SSH host" })}
            </Label>
            <Select
              id="tunnel-host"
              className="mt-1.5"
              value={draft.profileId}
              onChange={(event) => update({ profileId: event.target.value })}
            >
              <option value="">
                {t("tunnels.form.hostPlaceholder", { defaultValue: "Choose a saved host…" })}
              </option>
              {profiles.map((candidate) => (
                <option key={candidate.id} value={candidate.id}>
                  {profileLabel(candidate)}
                  {candidate.host ? ` — ${candidate.host}` : ""}
                </option>
              ))}
            </Select>
            {fieldError("hostRequired")}
            {profiles.length === 0 && (
              <p className="text-muted-foreground mt-1 text-xs">
                {t("tunnels.form.noHosts", {
                  defaultValue: "Save an SSH connection first, then come back to add a tunnel.",
                })}
              </p>
            )}
          </div>

          <div className="sm:col-span-2">
            <Label htmlFor="tunnel-name">{t("tunnels.form.name", { defaultValue: "Name" })}</Label>
            <Input
              id="tunnel-name"
              className="mt-1.5"
              value={effective.name}
              placeholder={t("tunnels.form.namePlaceholder", {
                defaultValue: "e.g. Production DB",
              })}
              onChange={(event) => {
                setNameOverride(event.target.value)
                setError(null)
              }}
            />
            {fieldError("nameRequired")}
          </div>

          <div>
            <Label htmlFor="tunnel-bind-host">{bindLabel}</Label>
            <Input
              id="tunnel-bind-host"
              className="mt-1.5 font-mono"
              value={draft.bindHost}
              placeholder="127.0.0.1"
              spellCheck={false}
              onChange={(event) => update({ bindHost: event.target.value })}
            />
            {fieldError("bindHostRequired")}
          </div>
          <div>
            <Label htmlFor="tunnel-bind-port">
              {t("tunnels.form.port", { defaultValue: "Port" })}
            </Label>
            <Input
              id="tunnel-bind-port"
              className="mt-1.5 font-mono"
              inputMode="numeric"
              value={bindPortText}
              placeholder="5432"
              onChange={(event) => {
                setBindPortText(event.target.value)
                setError(null)
              }}
            />
            {fieldError("bindPortInvalid")}
          </div>

          {draft.kind !== "dynamic" && (
            <>
              <div>
                <Label htmlFor="tunnel-dest-host">{destLabel}</Label>
                <Input
                  id="tunnel-dest-host"
                  className="mt-1.5 font-mono"
                  value={draft.destHost}
                  placeholder="db.internal"
                  spellCheck={false}
                  onChange={(event) => update({ destHost: event.target.value })}
                />
                {fieldError("destHostRequired")}
              </div>
              <div>
                <Label htmlFor="tunnel-dest-port">
                  {t("tunnels.form.port", { defaultValue: "Port" })}
                </Label>
                <Input
                  id="tunnel-dest-port"
                  className="mt-1.5 font-mono"
                  inputMode="numeric"
                  value={destPortText}
                  placeholder="5432"
                  onChange={(event) => {
                    setDestPortText(event.target.value)
                    setError(null)
                  }}
                />
                {fieldError("destPortInvalid")}
              </div>
            </>
          )}
        </div>

        {exposesBindAddress && (
          <div className="border-warning/40 bg-warning/10 flex items-start gap-2 rounded-md border px-3 py-2 text-xs">
            <AlertTriangle className="text-warning mt-0.5 size-3.5 shrink-0" />
            <span>
              {t("tunnels.form.exposedWarning", {
                defaultValue:
                  "This address is reachable by other devices on your network. Use 127.0.0.1 to keep the tunnel private to this device.",
              })}
            </span>
          </div>
        )}

        <div className="flex items-start justify-between gap-4 rounded-lg border px-3 py-2.5">
          <div className="min-w-0">
            <Label htmlFor="tunnel-auto-start">
              {t("tunnels.form.autoStart", { defaultValue: "Start when tTerm opens" })}
            </Label>
            <p className="text-muted-foreground mt-0.5 text-xs">
              {t("tunnels.form.autoStartDescription", {
                defaultValue:
                  "Only starts on its own if no password needs to be typed; otherwise it waits for you.",
              })}
            </p>
          </div>
          <Switch
            id="tunnel-auto-start"
            checked={draft.autoStart}
            onCheckedChange={(checked) => update({ autoStart: checked })}
          />
        </div>

        <div>
          <div className="text-muted-foreground mb-1.5 text-xs">
            {t("tunnels.form.equivalent", { defaultValue: "Equivalent OpenSSH command" })}
          </div>
          <code className="bg-muted block overflow-x-auto rounded-md px-3 py-2 font-mono text-xs whitespace-nowrap">
            {sshCommand}
          </code>
        </div>

        {saveError && (
          <p className="text-destructive text-sm" role="alert">
            {saveError}
          </p>
        )}
      </div>

      <DialogFooter className="border-border/80 border-t px-6 py-4">
        <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
          {t("common.cancel", { defaultValue: "Cancel" })}
        </Button>
        <Button type="button" onClick={handleSave} disabled={saving}>
          {saving && <Loader2 className="animate-spin" />}
          {t("common.save", { defaultValue: "Save" })}
        </Button>
      </DialogFooter>
    </>
  )
}

export const TunnelDialog: React.FC<TunnelDialogProps> = ({ open, onOpenChange, ...formProps }) => (
  <Dialog open={open} onOpenChange={onOpenChange}>
    <DialogContent className="gap-0 overflow-hidden p-0 sm:max-w-2xl">
      {/* Remounting per open gives every session a fresh draft. */}
      {open && (
        <TunnelForm
          key={formProps.tunnel?.id ?? "new"}
          onOpenChange={onOpenChange}
          {...formProps}
        />
      )}
    </DialogContent>
  </Dialog>
)
