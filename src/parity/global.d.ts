/** Globals the reference capture waits on. */
export {}

declare global {
  interface Window {
    /** Set once fonts and two frames are done; the capture waits for it. */
    __PARITY_READY__?: boolean
    /** Set when the entry point cannot render the requested screen. */
    __PARITY_ERROR__?: string
    /** Distinct commands the mocked invoke answered, sorted. */
    __PARITY_INVOKES__?: string[]
    /** Commands the screen called without a fixture, including caught ones. */
    __PARITY_MISSING__?: string[]
    /** Event listeners that had no declared fixture sequence. */
    __PARITY_MISSING_EVENTS__?: string[]
    __PARITY_ACTIONS_DONE__?: boolean
  }
}
