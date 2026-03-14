import { useCallback, useRef } from 'react';
import {Tab} from '../types/tab';

interface SessionData {
    tabs: Tab[];
    activeTabId: string | null;
    windowState?: {
        width: number;
        height: number;
        x?: number;
        y?: number;
        maximized?: boolean;
    };
    lastSaved: number;
}

const SESSION_STORAGE_KEY = 'tterm-session';
const SAVE_DEBOUNCE_MS = 1000; // 1 second debounce

export function useSessionPersistence() {
  // Save session data
  const saveSession = useCallback((tabs: Tab[], activeTabId: string | null) => {
    try {
      const sessionData: SessionData = {
        tabs: tabs.map(tab => ({
          ...tab,
          // Reset some runtime states
          isActive: tab.id === activeTabId,
        })),
        activeTabId,
        lastSaved: Date.now(),
      };

      localStorage.setItem(SESSION_STORAGE_KEY, JSON.stringify(sessionData));
    } catch (error) {
      console.error('Failed to save session:', error);
    }
  }, []);

  // Clear session data
  const clearSession = useCallback(() => {
    try {
      localStorage.removeItem(SESSION_STORAGE_KEY);
    } catch (error) {
      console.error('Failed to clear session:', error);
    }
  }, []);

  // Load session data
  const loadSession = useCallback((): SessionData | null => {
    try {
      const stored = localStorage.getItem(SESSION_STORAGE_KEY);
      if (!stored) return null;

      const sessionData: SessionData = JSON.parse(stored);

      // Check if data is expired (more than 7 days)
      const maxAge = 7 * 24 * 60 * 60 * 1000; // 7 days
      if (Date.now() - sessionData.lastSaved > maxAge) {
        clearSession();
        return null;
      }

      return sessionData;
    } catch (error) {
      console.error('Failed to load session:', error);
      return null;
    }
  }, [clearSession]);

  // Debounced save
  const timeoutRef = useRef<ReturnType<typeof setTimeout>>();
  
  const debouncedSave = useCallback((tabs: Tab[], activeTabId: string | null) => {
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current);
    }
    timeoutRef.current = setTimeout(() => {
      saveSession(tabs, activeTabId);
    }, SAVE_DEBOUNCE_MS);
  }, [saveSession]);

  return {
    saveSession: debouncedSave,
    loadSession,
    clearSession,
  };
}