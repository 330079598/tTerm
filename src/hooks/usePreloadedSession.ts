import { useCallback, useEffect, useRef } from "react"
import { useLatestRef } from "@/hooks/useLatestRef"

export function usePreloadedSession<T>(loadSession: () => Promise<T>): () => Promise<T> {
  const loadSessionRef = useRef(loadSession)
  const sessionPromiseRef = useRef<Promise<T> | null>(null)
  useLatestRef(loadSessionRef, loadSession)

  const getSession = useCallback(() => {
    sessionPromiseRef.current ??= loadSessionRef.current()
    return sessionPromiseRef.current
  }, [])

  useEffect(() => {
    void getSession()
  }, [getSession])

  return getSession
}
