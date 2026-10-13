import React, { useEffect, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { CheckCircle2, CircleAlert, Loader2, Save, Server, Terminal } from "lucide-react"
import { useTranslation } from "react-i18next"

import { useSavedPasswordAvailability } from "@/components/ConnectionDialog/useSavedPasswordAvailability"
import { useConnectionTest } from "@/components/ConnectionDialog/useConnectionTest"
import { useConnectionSubmit } from "@/components/ConnectionDialog/useConnectionSubmit"
import { SavedProfile } from "@/components/ProfilesPanel"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Separator } from "@/components/ui/separator"
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip"
import { useConfig } from "@/contexts/ConfigContext"
import { useUserVerification } from "@/contexts/UserVerificationContext"
import { useToast } from "@/hooks/use-toast"
import { cn } from "@/lib/utils"
import {
  buildInitialForm,
  getDefaultTitle,
} from "@/components/ConnectionDialog/connectionDialogUtils"
import { JumpHostFields } from "@/components/ConnectionDialog/JumpHostFields"
import { ProxyFields } from "@/components/ConnectionDialog/ProxyFields"
import { SshConnectionFields } from "@/components/ConnectionDialog/SshConnectionFields"
import { SessionCustomizationFields } from "@/components/ConnectionDialog/SessionCustomizationFields"
import { SudoAutofillFields } from "@/components/ConnectionDialog/SudoAutofillFields"
import { TerminalConnectionFields } from "@/components/ConnectionDialog/TerminalConnectionFields"
import { HostKeyPromptDialog } from "@/components/TerminalTab/HostKeyPromptDialog"
import { SshAuthPromptDialog } from "@/components/TerminalTab/SshAuthPromptDialog"
import { getSshConnectionProgressLabel } from "@/components/TerminalTab/terminalTabUtils"
import {
  ConnectionDialogContentProps,
  ConnectionDialogProps,
  ConnectionForm,
  connectionTypes,
} from "@/components/ConnectionDialog/types"
import { getJumpHostErrorKey } from "@/components/ConnectionDialog/jumpHostPayload"

const connectionTypeIcons = {
  terminal: Terminal,
  ssh: Server,
} as const

const ConnectionDialogContent: React.FC<ConnectionDialogContentProps> = ({
  onClose,
  onConnect,
  editProfile,
  duplicateProfile,
  draftProfile,
  saveOnly: saveOnlyProp = false,
  onSaved,
  typedPasswordTabId,
  canReconnect = false,
  config,
  saveConfig,
}) => {
  const { t } = useTranslation()
  const { toast } = useToast()
  const { withVaultUnlock } = useUserVerification()
  const [form, setForm] = useState<ConnectionForm>(() => {
    const initialForm = buildInitialForm(editProfile ?? duplicateProfile ?? draftProfile, config)
    if (duplicateProfile) {
      initialForm.title = t("profiles.copyName", { name: duplicateProfile.name })
    }
    return initialForm
  })
  // The parent keys this component by profile/mode, so a different profile
  // remounts it with fresh form state instead of resetting in an effect.
  const [draftProfileId] = useState(() => editProfile?.id ?? crypto.randomUUID())
  const [existingGroups, setExistingGroups] = useState<string[]>([])
  const [showGroupDropdown, setShowGroupDropdown] = useState(false)
  const [nameError, setNameError] = useState<string | null>(null)
  const [allProfiles, setAllProfiles] = useState<SavedProfile[]>([])
  const [jumpHostErrors, setJumpHostErrors] = useState<Record<string, string>>({})
  const {
    savedPasswordAvailable,
    typedPasswordAvailable,
    savedSudoPasswordAvailable,
    savedJumpPasswordKeys,
  } = useSavedPasswordAvailability({ editProfile, draftProfile, typedPasswordTabId, form, setForm })
  const matchingGroups = existingGroups.filter((group) =>
    group.toLowerCase().includes(form.group.toLowerCase())
  )
  const sshProfileId = editProfile?.id ?? draftProfileId
  // Editing a profile changes what its tabs connect with; it never opens another tab.
  const saveOnly = saveOnlyProp || Boolean(editProfile)

  useEffect(() => {
    Promise.all([invoke<SavedProfile[]>("list_profiles"), invoke<string[]>("list_profile_groups")])
      .then(([profiles, configuredGroups]) => {
        setAllProfiles(profiles)
        const groups = [
          ...new Set([
            ...configuredGroups,
            ...profiles.map((profile) => profile.group).filter(Boolean),
          ]),
        ].sort((left, right) => left.localeCompare(right))
        setExistingGroups(groups)
      })
      .catch(() => {})
  }, [])

  const { handleSubmit } = useConnectionSubmit({
    form,
    setForm,
    setNameError,
    setJumpHostErrors,
    toast,
    t,
    editProfile,
    sshProfileId,
    savedPasswordAvailable,
    savedJumpPasswordKeys,
    allProfiles,
    typedPasswordTabId,
    saveOnly,
    onSaved,
    onClose,
    onConnect,
    saveConfig,
  })

  const {
    isTesting,
    testProgress,
    testResult,
    testHostKeyPrompt,
    setTestHostKeyPrompt,
    handleTestConnection,
  } = useConnectionTest({
    form,
    setJumpHostErrors,
    toast,
    t,
    sshProfileId,
    savedPasswordAvailable,
    editProfile,
    withVaultUnlock,
    typedPasswordTabId,
  })

  const isSsh = form.type === "ssh"
  const isTerminal = form.type === "terminal"

  return (
    <>
      <DialogContent
        className="flex max-h-[85vh] flex-col overflow-hidden sm:max-w-[600px]"
        onInteractOutside={(event) => event.preventDefault()}
      >
        <DialogHeader>
          <DialogTitle>
            {editProfile
              ? t("profiles.editTitle")
              : saveOnly
                ? t("quickConnect.saveTitle")
                : duplicateProfile
                  ? t("profiles.copyTitle")
                  : t("connection.newConnection")}
          </DialogTitle>
        </DialogHeader>

        <form onSubmit={handleSubmit} className="flex-1 space-y-5 overflow-y-auto px-1">
          <div hidden={saveOnly}>
            <Label className="mb-2 block">{t("connection.type")}</Label>
            <div className="grid grid-cols-2 gap-2">
              {connectionTypes.map(({ type, label }) => {
                const Icon = connectionTypeIcons[type]

                return (
                  <Button
                    key={type}
                    type="button"
                    variant={form.type === type ? "default" : "outline"}
                    onClick={() => setForm((current) => ({ ...current, type }))}
                    className={cn(
                      "h-auto justify-start gap-2 px-3 py-2.5 text-sm transition-colors",
                      form.type === type ? "shadow-none" : "text-muted-foreground"
                    )}
                  >
                    <Icon size={16} />
                    {label}
                  </Button>
                )
              })}
            </div>
          </div>

          {!saveOnly && <Separator />}

          {!isSsh && (
            <div>
              <Label htmlFor="conn-title" className="mb-1.5 block">
                {t("connection.title")}
              </Label>
              <Input
                id="conn-title"
                value={form.title}
                onChange={(e) => {
                  setForm((current) => ({ ...current, title: e.target.value }))
                  setNameError(null)
                }}
                placeholder={getDefaultTitle(form.type, form)}
              />
              {nameError && <p className="text-destructive mt-1 text-xs">{nameError}</p>}
            </div>
          )}

          {isTerminal && <TerminalConnectionFields form={form} setForm={setForm} />}

          {isSsh && (
            <>
              <SshConnectionFields
                form={form}
                setForm={setForm}
                savedPasswordAvailable={savedPasswordAvailable || typedPasswordAvailable}
                savedPasswordTyped={typedPasswordAvailable}
                matchingGroups={matchingGroups}
                nameError={nameError}
                setNameError={setNameError}
                showGroupDropdown={showGroupDropdown}
                setShowGroupDropdown={setShowGroupDropdown}
                titlePlaceholder={getDefaultTitle(form.type, form)}
              />
              <JumpHostFields
                form={form}
                setForm={setForm}
                savedJumpPasswordKeys={savedJumpPasswordKeys}
                errors={jumpHostErrors}
                onClearError={(jumpId, field) =>
                  setJumpHostErrors((current) => {
                    const key = getJumpHostErrorKey(jumpId, field)
                    if (!current[key]) return current
                    const { [key]: _, ...remaining } = current
                    return remaining
                  })
                }
              />
              <ProxyFields form={form} setForm={setForm} />
              <SudoAutofillFields
                form={form}
                setForm={setForm}
                savedSudoPasswordAvailable={savedSudoPasswordAvailable}
              />
              <SessionCustomizationFields form={form} setForm={setForm} />
            </>
          )}

          {isSsh && testProgress && (
            <div
              role="status"
              aria-live="polite"
              className="border-border bg-muted/35 text-muted-foreground rounded-md border px-3 py-2 text-xs"
            >
              {getSshConnectionProgressLabel(testProgress, t)}
            </div>
          )}

          {isSsh && !testProgress && testResult && (
            <div
              role={testResult.status === "error" ? "alert" : "status"}
              aria-live="polite"
              className={cn(
                "flex items-start gap-2 rounded-md border px-3 py-2 text-xs",
                testResult.status === "success"
                  ? "border-green-500/40 bg-green-500/10 text-green-700 dark:text-green-300"
                  : "border-destructive/40 bg-destructive/10 text-destructive"
              )}
            >
              {testResult.status === "success" ? (
                <CheckCircle2 className="mt-0.5 size-4 shrink-0" />
              ) : (
                <CircleAlert className="mt-0.5 size-4 shrink-0" />
              )}
              <span>
                <span className="font-medium">
                  {testResult.status === "success"
                    ? t("profiles.testSuccess")
                    : t("profiles.testFailed")}
                  :{" "}
                </span>
                {testResult.message}
              </span>
            </div>
          )}

          <DialogFooter>
            <Button type="button" variant="ghost" onClick={onClose}>
              {t("connection.cancel")}
            </Button>
            {isSsh && (
              <Button
                key={isTesting ? "testing" : "test"}
                type="button"
                variant="outline"
                onClick={handleTestConnection}
                disabled={isTesting || !form.host.trim() || !form.username.trim()}
                className="min-w-[100px]"
              >
                {isTesting && <Loader2 className="animate-spin" />}
                {isTesting ? t("profiles.testing") : t("profiles.test")}
              </Button>
            )}
            <TooltipProvider>
              {saveOnly ? (
                <>
                  {canReconnect && (
                    <Tooltip>
                      <TooltipTrigger asChild>
                        <Button
                          type="submit"
                          variant="outline"
                          data-action="save-reconnect"
                          disabled={!form.host.trim()}
                        >
                          {t("profiles.saveAndReconnect")}
                        </Button>
                      </TooltipTrigger>
                      <TooltipContent>{t("profiles.saveAndReconnectDescription")}</TooltipContent>
                    </Tooltip>
                  )}
                  <Button type="submit" data-action="save" disabled={!form.host.trim()}>
                    <Save size={14} />
                    {t("quickConnect.save")}
                  </Button>
                </>
              ) : (
                <>
                  {isSsh && (
                    <Tooltip>
                      <TooltipTrigger asChild>
                        <Button type="submit" variant="outline" data-action="save">
                          <Save size={14} />
                          {t("connection.saveAndConnect")}
                        </Button>
                      </TooltipTrigger>
                      <TooltipContent>{t("connection.saveAndConnectDescription")}</TooltipContent>
                    </Tooltip>
                  )}
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <Button type="submit" data-action="connect">
                        {t("connection.connect")}
                      </Button>
                    </TooltipTrigger>
                    <TooltipContent>{t("connection.connectDescription")}</TooltipContent>
                  </Tooltip>
                </>
              )}
            </TooltipProvider>
          </DialogFooter>
        </form>
      </DialogContent>
      <HostKeyPromptDialog
        hostKeyPrompt={testHostKeyPrompt}
        setHostKeyPrompt={setTestHostKeyPrompt}
      />
      <SshAuthPromptDialog tabId={`test-${sshProfileId}`} />
    </>
  )
}

export const ConnectionDialog: React.FC<ConnectionDialogProps> = ({
  isOpen,
  onClose,
  onConnect,
  editProfile,
  duplicateProfile,
  draftProfile,
  saveOnly,
  onSaved,
  typedPasswordTabId,
  canReconnect,
}) => {
  const { config, saveConfig } = useConfig()
  const dialogKey = [
    editProfile?.id ?? duplicateProfile?.id ?? "new",
    duplicateProfile ? "duplicate" : draftProfile ? `draft:${draftProfile.name}` : "edit",
    config.terminal_shell,
    config.terminal_shell_custom_path,
    config.terminal_shell_custom_args,
  ].join("::")

  return (
    <Dialog
      open={isOpen}
      onOpenChange={(open) => {
        if (!open) onClose()
      }}
    >
      {isOpen ? (
        <ConnectionDialogContent
          key={dialogKey}
          onClose={onClose}
          onConnect={onConnect}
          editProfile={editProfile}
          duplicateProfile={duplicateProfile}
          draftProfile={draftProfile}
          saveOnly={saveOnly}
          onSaved={onSaved}
          typedPasswordTabId={typedPasswordTabId}
          canReconnect={canReconnect}
          config={config}
          saveConfig={saveConfig}
        />
      ) : null}
    </Dialog>
  )
}
