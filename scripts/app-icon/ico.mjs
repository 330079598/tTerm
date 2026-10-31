// Merges two Windows .ico files so the small entries come from one drawing
// and the large entries from another. `tauri icon` builds an .ico from a
// single source, and the app icon uses a simplified drawing for 32 px and
// below.

const HEADER_SIZE = 6
const ENTRY_SIZE = 16

function readEntries(buffer) {
  const count = buffer.readUInt16LE(4)
  return Array.from({ length: count }, (_, index) => {
    const start = HEADER_SIZE + index * ENTRY_SIZE
    const size = buffer.readUInt32LE(start + 8)
    const offset = buffer.readUInt32LE(start + 12)
    return {
      directory: buffer.subarray(start, start + ENTRY_SIZE),
      // A width byte of 0 means 256 px.
      width: buffer[start] || 256,
      image: buffer.subarray(offset, offset + size),
    }
  })
}

/** Returns an .ico with the entries at or below `maxSmall` px from `small` and the rest from `large`. */
export function mergeIco(large, small, maxSmall) {
  const entries = [
    ...readEntries(small).filter((entry) => entry.width <= maxSmall),
    ...readEntries(large).filter((entry) => entry.width > maxSmall),
  ].sort((a, b) => a.width - b.width)

  const header = Buffer.alloc(HEADER_SIZE)
  header.writeUInt16LE(1, 2)
  header.writeUInt16LE(entries.length, 4)

  let offset = HEADER_SIZE + entries.length * ENTRY_SIZE
  const directories = entries.map((entry) => {
    const directory = Buffer.from(entry.directory)
    directory.writeUInt32LE(entry.image.length, 8)
    directory.writeUInt32LE(offset, 12)
    offset += entry.image.length
    return directory
  })
  return Buffer.concat([header, ...directories, ...entries.map((entry) => entry.image)])
}
