// Over a slow link every key waits a round trip for its echo, so typing at a
// remote shell lags. The predictor guesses the echo of plain typing and
// backspace on the cursor's line, the way mosh does, and checks each guess
// against the terminal once the remote's output arrives. A guess is only
// shown once the remote has echoed a key typed on the same line, so nothing
// typed at a prompt that does not echo, such as a password prompt, is shown.
// Only the echo of a typed character counts: a backspace's echo, `\b \b`, is
// also what a password prompt that masks with `*` sends, and a line that
// echoed a key as something else is masking it, so nothing more on it is shown.
//
// A key the predictor can't guess, such as Enter or Tab, leaves it not knowing
// where typing goes next, and keys sent before it may still be on their way.
// Keys typed after it are placed tentatively: they are not shown, and when the
// line does not match them they are moved to where it shows typing goes,
// rather than taken for a masking prompt, until one is echoed where placed.

/** What the predictor reads of the terminal. Rows count from the top of the scrollback. */
export interface PredictionScreen {
  cursorRow: number
  cursorCol: number
  cols: number
  /** The character in a cell, a space when the cell is empty. */
  charAt(row: number, col: number): string
}

/** A cell the predicted echo changes. */
export interface PredictedCell {
  col: number
  /** A space for a character the predicted echo erases. */
  char: string
  /** The echo is overdue, so the guess is marked as one. */
  stalled: boolean
}

/** The predicted echo of what is typed but not yet echoed. */
export interface PredictionView {
  row: number
  cells: PredictedCell[]
  cursorCol: number
}

/** A guess is marked as one once its echo is this late, or two round trips. */
const STALL_MS = 500
/** Guesses still not echoed this long after are dropped, or four round trips. */
const TIMEOUT_MS = 2000
/** The line stopped matching the guesses this long ago, so they were wrong. */
const DOUBT_MS = 150

interface Keystroke {
  col: number
  /** What the cell holds once the key is echoed. */
  value: string
  cursorAfter: number
  at: number
  /** A backspace, whose echo looks the same at a prompt that masks what is typed. */
  erases: boolean
}

const isPredictable = (char: string) => char >= " " && char <= "~"

/** `data` ends the line typed on: Enter, or Ctrl+C or Ctrl+D giving it up. */
export const endsLine = (data: string) => [..."\r\n\x03\x04"].some((key) => data.includes(key))

/** Guesses the echo of keys typed at a remote shell's line editor. */
export class EchoPredictor {
  /** The row typed on, or null until a key is typed. */
  private row: number | null = null
  /** Backspace stops here, where typing on the row started. */
  private lineStart = 0
  /** The cursor column before the first key not yet echoed. */
  private base = 0
  private pending: Keystroke[] = []
  /** The remote echoed a key typed on this row. */
  private confirmed = false
  /** When the line stopped matching the guesses. */
  private doubtSince: number | null = null
  private srtt: number | null = null
  private slow = false
  /**
   * A row that echoed a key as something other than the key, as a password
   * prompt masking with `*` does; nothing typed on it is shown until the line
   * is ended.
   */
  private maskedRow: number | null = null
  /**
   * The guesses are placed tentatively, since a key that can't be guessed
   * went out after the last one echoed where it was placed.
   */
  private tentative = false
  /**
   * What a tentative guess was echoed as instead, and on which row: a key
   * still on its way or a prompt masking with `*`. An echo of a key typed as
   * that character does not show that the row echoes typing.
   */
  private suspect: { row: number; char: string } | null = null

  constructor(private readonly now: () => number = Date.now) {}

  /** The smoothed time from a key to its echo, once one was measured. */
  get roundTripMs(): number | null {
    return this.srtt
  }

  /**
   * The round trip is over `thresholdMs`. Once it is, it stays so until the
   * round trip drops below two thirds of that, so a link near the threshold
   * does not turn guessing on and off.
   */
  slowerThan(thresholdMs: number): boolean {
    if (this.srtt === null) return false
    if (this.srtt > thresholdMs) this.slow = true
    else if (this.srtt < (thresholdMs * 2) / 3) this.slow = false
    return this.slow
  }

  /** The row being predicted on, or null. */
  get activeRow(): number | null {
    return this.row
  }

  /** Keys are waiting for their echo, or the line is in doubt. */
  get busy(): boolean {
    return this.pending.length > 0 || this.doubtSince !== null
  }

  /**
   * Forgets every guess; the next row typed on must echo again before guesses
   * show. Keys sent meanwhile may still be on their way, so the guesses after
   * it are placed tentatively.
   */
  reset() {
    this.clear()
    this.tentative = true
  }

  /** Notes `data` typed, before it is sent. */
  type(data: string, screen: PredictionScreen) {
    if (endsLine(data)) {
      this.maskedRow = null
      this.suspect = null
    }
    for (const char of data) {
      const typed =
        char === "\x7f" || char === "\b"
          ? this.erase(screen)
          : isPredictable(char) && this.insert(char, screen)
      if (!typed) {
        this.reset()
        return
      }
    }
  }

  /** Checks the guesses against the terminal once the remote's output is parsed. */
  reconcile(screen: PredictionScreen) {
    if (this.row === null || this.pending.length === 0) return
    const row = this.row
    const echoed =
      screen.cursorRow === row
        ? this.pending.findIndex(
            (key) =>
              key.cursorAfter === screen.cursorCol && screen.charAt(row, key.col) === key.value
          )
        : -1
    if (echoed === -1) {
      if (this.tentative) {
        this.place(row, screen)
        return
      }
      if (this.echoedAs(row, screen) !== null) {
        this.clear()
        this.maskedRow = row
        return
      }
      // Nothing echoed yet. A shell that redraws its line can be caught
      // halfway through, so a line that does not match is only doubted.
      const waiting = screen.cursorRow === row && screen.cursorCol === this.base
      this.doubtSince = waiting ? null : (this.doubtSince ?? this.now())
      return
    }
    const key = this.pending[echoed]
    this.sample(this.now() - key.at)
    const masked =
      row === this.maskedRow || (this.suspect?.row === row && this.suspect.char === key.value)
    if (!key.erases && !masked) this.confirmed = true
    this.tentative = false
    this.doubtSince = null
    this.base = key.cursorAfter
    this.pending = this.pending.slice(echoed + 1)
  }

  /** Drops guesses the remote never confirmed. */
  expire(screen: PredictionScreen) {
    const now = this.now()
    if (this.doubtSince !== null && now - this.doubtSince > DOUBT_MS) {
      this.reset()
      return
    }
    const oldest = this.pending[0]
    if (!oldest || now - oldest.at <= Math.max(TIMEOUT_MS, 4 * (this.srtt ?? 0))) return
    if (this.matches(screen)) {
      // A key and its backspace both echoed leave the cursor where it was,
      // which looks like no echo at all; the line is right, so guessing goes on.
      this.base = screen.cursorCol
      this.pending = []
    } else {
      this.clear()
    }
    // Whatever was on its way when these keys were typed has long arrived.
    this.tentative = false
  }

  /** The guesses to show, or null while none may be shown. */
  view(): PredictionView | null {
    if (this.row === null || !this.confirmed || this.tentative || this.doubtSince !== null) {
      return null
    }
    const last = this.pending[this.pending.length - 1]
    if (!last) return null
    const now = this.now()
    const stallMs = Math.max(STALL_MS, 2 * (this.srtt ?? 0))
    const cells = new Map<number, PredictedCell>()
    for (const key of this.pending) {
      cells.set(key.col, { col: key.col, char: key.value, stalled: now - key.at > stallMs })
    }
    return { row: this.row, cells: [...cells.values()], cursorCol: last.cursorAfter }
  }

  private clear() {
    this.row = null
    this.pending = []
    this.confirmed = false
    this.doubtSince = null
  }

  /**
   * Moves the tentative guesses to where the line shows typing goes: from the
   * cursor, less those of them it already shows just before the cursor.
   */
  private place(row: number, screen: PredictionScreen) {
    const other = this.echoedAs(row, screen)
    if (other !== null) this.suspect = { row, char: other }
    const keys = this.pending.slice(this.echoedBefore(screen))
    this.pending = []
    // A new row is started over by nextCol.
    if (screen.cursorRow !== row) this.row = null
    for (const key of keys) {
      const placed = key.erases
        ? this.erase(screen, key.at)
        : this.insert(key.value, screen, key.at)
      if (!placed) {
        this.reset()
        return
      }
    }
  }

  /** How many of the guesses, from the first, the line shows just before the cursor. */
  private echoedBefore(screen: PredictionScreen): number {
    const firstErase = this.pending.findIndex((key) => key.erases)
    const typed = firstErase === -1 ? this.pending.length : firstErase
    for (let count = Math.min(typed, screen.cursorCol); count > 0; count -= 1) {
      const start = screen.cursorCol - count
      const shown = this.pending
        .slice(0, count)
        .every((key, i) => screen.charAt(screen.cursorRow, start + i) === key.value)
      if (shown) return count
    }
    return 0
  }

  private insert(char: string, screen: PredictionScreen, at = this.now()): boolean {
    const col = this.nextCol(screen)
    // A key at the last column leaves the cursor waiting to wrap.
    if (col >= screen.cols - 1) return false
    // Typing into text, rather than at its end, shifts what follows. Typing
    // the next character of a shell's grayed-out suggestion is at its end.
    const current = this.valueAt(col, screen)
    if (current !== " " && current !== char) return false
    this.pending.push({ col, value: char, cursorAfter: col + 1, at, erases: false })
    return true
  }

  private erase(screen: PredictionScreen, at = this.now()): boolean {
    const col = this.nextCol(screen)
    // Before the row's typing lies the prompt, which the shell keeps.
    if (col <= this.lineStart) return false
    this.pending.push({
      col: col - 1,
      value: " ",
      cursorAfter: col - 1,
      at,
      erases: true,
    })
    return true
  }

  /** Where the next key lands: after the last guess, or at the cursor. */
  private nextCol(screen: PredictionScreen): number {
    const last = this.pending[this.pending.length - 1]
    if (last) return last.cursorAfter
    if (this.row !== screen.cursorRow) {
      this.clear()
      this.row = screen.cursorRow
      this.lineStart = screen.cursorCol
    }
    this.lineStart = Math.min(this.lineStart, screen.cursorCol)
    this.base = screen.cursorCol
    this.doubtSince = null
    return screen.cursorCol
  }

  private valueAt(col: number, screen: PredictionScreen): string {
    for (let i = this.pending.length - 1; i >= 0; i -= 1) {
      if (this.pending[i].col === col) return this.pending[i].value
    }
    return screen.charAt(this.row ?? screen.cursorRow, col)
  }

  /**
   * What the cell of a typed key holds once the cursor moved past it, when
   * that is another character, or null.
   */
  private echoedAs(row: number, screen: PredictionScreen): string | null {
    if (screen.cursorRow !== row) return null
    const key = this.pending.find(
      (key) =>
        !key.erases &&
        key.cursorAfter === screen.cursorCol &&
        screen.charAt(row, key.col) !== key.value
    )
    return key ? screen.charAt(row, key.col) : null
  }

  /** The terminal already shows what the guesses predict. */
  private matches(screen: PredictionScreen): boolean {
    const last = this.pending[this.pending.length - 1]
    if (this.row === null || !last) return true
    if (screen.cursorRow !== this.row || screen.cursorCol !== last.cursorAfter) return false
    const row = this.row
    return this.pending.every(
      (key) => screen.charAt(row, key.col) === this.valueAt(key.col, screen)
    )
  }

  private sample(rttMs: number) {
    this.srtt = this.srtt === null ? rttMs : this.srtt + (rttMs - this.srtt) / 8
  }
}
