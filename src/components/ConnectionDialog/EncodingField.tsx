import React from "react"
import { useTranslation } from "react-i18next"

import { normalizeTerminalEncoding } from "@/components/ConnectionDialog/connectionDialogUtils"
import { type ConnectionForm, TERMINAL_ENCODINGS } from "@/components/ConnectionDialog/types"
import { Label } from "@/components/ui/label"
import { Select } from "@/components/ui/select"

interface EncodingFieldProps {
  form: ConnectionForm
  setForm: React.Dispatch<React.SetStateAction<ConnectionForm>>
  /** What the charset is of, under the field. */
  description: string
}

/** The charset a connection's text is sent and shown in. */
export const EncodingField: React.FC<EncodingFieldProps> = ({ form, setForm, description }) => {
  const { t } = useTranslation()
  return (
    <div>
      <Label htmlFor="conn-encoding" className="mb-1.5 block">
        {t("connection.encoding")}
      </Label>
      <Select
        id="conn-encoding"
        value={form.encoding}
        onChange={(e) =>
          setForm((current) => ({
            ...current,
            encoding: normalizeTerminalEncoding(e.target.value),
          }))
        }
      >
        {TERMINAL_ENCODINGS.map((encoding) => (
          <option key={encoding} value={encoding}>
            {t(`connection.encodingOptions.${encoding}`)}
          </option>
        ))}
      </Select>
      <p className="text-muted-foreground mt-1 text-xs">{description}</p>
    </div>
  )
}
