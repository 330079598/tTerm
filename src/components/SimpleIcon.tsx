/** A one-path logo on Simple Icons' 24×24 grid, drawn in its color or the text color. */
export function SimpleIcon({
  path,
  className,
  size,
  color,
}: {
  path: string
  className?: string
  size: number
  /** A `#rrggbb` color, or "" for the text color. */
  color: string
}) {
  return (
    <svg
      className={className}
      viewBox="0 0 24 24"
      width={size}
      height={size}
      fill="currentColor"
      style={color ? { color } : undefined}
      aria-hidden="true"
    >
      <path d={path} />
    </svg>
  )
}
