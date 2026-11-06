/** Props every parity screen receives from the parity entry point. */
export interface ParityScreenProps {
  /**
   * Called once the screen has everything the screenshot needs. The entry
   * point waits for fonts and two animation frames before declaring the page
   * ready, so an async fixture load must delay this call.
   */
  onReady: () => void
}
