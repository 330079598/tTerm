import { PasswordInput } from "@/components/ui/password-input"
import { Select } from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"
import type { ParityScreenProps } from "@/parity/types"
import { useEffect } from "react"

export function PasswordInputPrimitive({ onReady }: ParityScreenProps) {
  useEffect(() => onReady(), [onReady])
  return (
    <main className="bg-background text-foreground h-full w-full space-y-4 p-8">
      <PasswordInput
        data-parity="ui.password.default"
        aria-label="Password"
        placeholder="Password"
      />
      <PasswordInput
        data-parity="ui.password.masked"
        aria-label="Masked"
        value="secret-value"
        readOnly
      />
      <PasswordInput
        data-parity="ui.password.disabled"
        aria-label="Disabled"
        value="Disabled"
        disabled
        readOnly
      />
    </main>
  )
}

export function SelectPrimitive({ onReady }: ParityScreenProps) {
  useEffect(() => onReady(), [onReady])
  return (
    <main className="bg-background text-foreground h-full w-full space-y-4 p-8">
      <Select data-parity="ui.select.default" defaultValue="one">
        <option value="one">One</option>
        <option value="two">Two</option>
      </Select>
      <Select data-parity="ui.select.focus" autoFocus defaultValue="two">
        <option value="one">One</option>
        <option value="two">Two</option>
      </Select>
      <Select data-parity="ui.select.disabled" disabled defaultValue="one">
        <option value="one">One</option>
        <option value="two">Two</option>
      </Select>
      <Select data-parity="ui.select.long" defaultValue="long">
        <option value="long">A very long selected value that truncates</option>
      </Select>
    </main>
  )
}

export function SwitchPrimitive({ onReady }: ParityScreenProps) {
  useEffect(() => onReady(), [onReady])
  return (
    <main className="bg-background text-foreground flex h-full w-full flex-col gap-4 p-8">
      <Switch data-parity="ui.switch.checked" checked onCheckedChange={() => {}} />
      <Switch data-parity="ui.switch.unchecked" checked={false} onCheckedChange={() => {}} />
      <Switch data-parity="ui.switch.focus" checked autoFocus onCheckedChange={() => {}} />
      <Switch data-parity="ui.switch.disabled" checked disabled onCheckedChange={() => {}} />
    </main>
  )
}
