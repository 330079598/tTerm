import React, { useState } from "react"
import { RefreshCw } from "lucide-react"
import { useTranslation } from "react-i18next"

import { useSerialPorts } from "@/components/ConnectionDialog/useSerialPorts"
import type { ConnectionForm } from "@/components/ConnectionDialog/types"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Select } from "@/components/ui/select"
import {
  formatUsbId,
  SERIAL_BAUD_RATES,
  SERIAL_DATA_BITS,
  SERIAL_FLOW_CONTROLS,
  SERIAL_LINE_ENDINGS,
  SERIAL_PARITIES,
  SERIAL_STOP_BITS,
  serialPortDescription,
  withSerialPort,
} from "@/lib/serial"
import { cn } from "@/lib/utils"
import type { SerialPortEntry, SerialSettings } from "@/types/serial"

/** The port picker's choice that reveals a field for typing a port's name. */
const OTHER_PORT = "\0other"

interface SerialConnectionFieldsProps {
  form: ConnectionForm
  setForm: React.Dispatch<React.SetStateAction<ConnectionForm>>
}

function portLabel(entry: SerialPortEntry): string {
  const description = serialPortDescription(entry)
  const usb = entry.usb ? ` (${formatUsbId(entry.usb)})` : ""
  return description ? `${entry.path} — ${description}${usb}` : `${entry.path}${usb}`
}

export const SerialConnectionFields: React.FC<SerialConnectionFieldsProps> = ({
  form,
  setForm,
}) => {
  const { t } = useTranslation()
  const { ports, loading, error, refresh } = useSerialPorts()
  const serial = form.serial
  const listed = ports.some((port) => port.path === serial.port)
  const [choosingOther, setChoosingOther] = useState(false)
  const update = (patch: Partial<SerialSettings>) =>
    setForm((current) => ({ ...current, serial: { ...current.serial, ...patch } }))

  const choosePort = (value: string) => {
    setChoosingOther(value === OTHER_PORT)
    const entry = ports.find((port) => port.path === value)
    if (entry) update(withSerialPort(serial, entry))
  }

  return (
    <>
      <div>
        <Label htmlFor="conn-serial-port" className="mb-1.5 block">
          {t("connection.serial.port")}
        </Label>
        <div className="flex gap-2">
          <Select
            id="conn-serial-port"
            className="flex-1"
            value={choosingOther ? OTHER_PORT : serial.port}
            onChange={(event) => choosePort(event.target.value)}
            required={!choosingOther}
          >
            {!serial.port && !choosingOther && (
              <option value="" disabled>
                {ports.length === 0
                  ? t("connection.serial.noPorts")
                  : t("connection.serial.choosePort")}
              </option>
            )}
            {ports.map((port) => (
              <option key={port.path} value={port.path}>
                {portLabel(port)}
              </option>
            ))}
            {/* A saved port that isn't plugged in still shows under its name. */}
            {serial.port && !listed && !choosingOther && (
              <option value={serial.port}>
                {t("connection.serial.notConnected", { port: serial.port })}
              </option>
            )}
            <option value={OTHER_PORT}>{t("connection.serial.otherPort")}</option>
          </Select>
          <Button
            type="button"
            variant="outline"
            size="icon"
            onClick={refresh}
            disabled={loading}
            aria-label={t("connection.serial.refresh")}
            title={t("connection.serial.refresh")}
          >
            <RefreshCw size={14} className={cn(loading && "animate-spin")} />
          </Button>
        </div>
        {choosingOther && (
          <Input
            id="conn-serial-port-path"
            className="mt-2"
            value={serial.port}
            onChange={(event) => update({ port: event.target.value, usb: undefined })}
            placeholder={t("connection.serial.portPlaceholder")}
            required
          />
        )}
        {error && (
          <p className="text-destructive mt-1 text-xs">
            {t("connection.serial.listFailed", { error })}
          </p>
        )}
      </div>

      <div className="grid grid-cols-2 gap-3 sm:grid-cols-3">
        <div>
          <Label htmlFor="conn-serial-baud" className="mb-1.5 block">
            {t("connection.serial.baudRate")}
          </Label>
          <Input
            id="conn-serial-baud"
            type="number"
            min={1}
            list="conn-serial-baud-rates"
            value={serial.baudRate || ""}
            onChange={(event) => update({ baudRate: Number(event.target.value) })}
            required
          />
          <datalist id="conn-serial-baud-rates">
            {SERIAL_BAUD_RATES.map((rate) => (
              <option key={rate} value={rate} />
            ))}
          </datalist>
        </div>
        <div>
          <Label htmlFor="conn-serial-data-bits" className="mb-1.5 block">
            {t("connection.serial.dataBits")}
          </Label>
          <Select
            id="conn-serial-data-bits"
            value={serial.dataBits}
            onChange={(event) => update({ dataBits: Number(event.target.value) })}
          >
            {SERIAL_DATA_BITS.map((bits) => (
              <option key={bits} value={bits}>
                {bits}
              </option>
            ))}
          </Select>
        </div>
        <div>
          <Label htmlFor="conn-serial-parity" className="mb-1.5 block">
            {t("connection.serial.parity")}
          </Label>
          <Select
            id="conn-serial-parity"
            value={serial.parity}
            onChange={(event) => update({ parity: event.target.value as SerialSettings["parity"] })}
          >
            {SERIAL_PARITIES.map((parity) => (
              <option key={parity} value={parity}>
                {t(`connection.serial.parityOptions.${parity}`)}
              </option>
            ))}
          </Select>
        </div>
        <div>
          <Label htmlFor="conn-serial-stop-bits" className="mb-1.5 block">
            {t("connection.serial.stopBits")}
          </Label>
          <Select
            id="conn-serial-stop-bits"
            value={serial.stopBits}
            onChange={(event) => update({ stopBits: Number(event.target.value) })}
          >
            {SERIAL_STOP_BITS.map((bits) => (
              <option key={bits} value={bits}>
                {bits}
              </option>
            ))}
          </Select>
        </div>
        <div>
          <Label htmlFor="conn-serial-flow" className="mb-1.5 block">
            {t("connection.serial.flowControl")}
          </Label>
          <Select
            id="conn-serial-flow"
            value={serial.flowControl}
            onChange={(event) =>
              update({ flowControl: event.target.value as SerialSettings["flowControl"] })
            }
          >
            {SERIAL_FLOW_CONTROLS.map((flow) => (
              <option key={flow} value={flow}>
                {t(`connection.serial.flowControlOptions.${flow}`)}
              </option>
            ))}
          </Select>
        </div>
        <div>
          <Label htmlFor="conn-serial-line-ending" className="mb-1.5 block">
            {t("connection.serial.lineEnding")}
          </Label>
          <Select
            id="conn-serial-line-ending"
            value={serial.lineEnding}
            onChange={(event) =>
              update({ lineEnding: event.target.value as SerialSettings["lineEnding"] })
            }
          >
            {SERIAL_LINE_ENDINGS.map((ending) => (
              <option key={ending} value={ending}>
                {t(`connection.serial.lineEndingOptions.${ending}`)}
              </option>
            ))}
          </Select>
        </div>
      </div>

      {(
        [
          ["localEcho", "conn-serial-local-echo"],
          ["timestamps", "conn-serial-timestamps"],
        ] as const
      ).map(([key, id]) => (
        <div key={key} className="flex items-start gap-2">
          <Checkbox
            id={id}
            checked={serial[key]}
            onCheckedChange={(checked) => update({ [key]: checked })}
          />
          <div className="space-y-0.5">
            <Label htmlFor={id} className="text-sm font-normal">
              {t(`connection.serial.${key}`)}
            </Label>
            <p className="text-muted-foreground text-xs">{t(`connection.serial.${key}Desc`)}</p>
          </div>
        </div>
      ))}
    </>
  )
}
