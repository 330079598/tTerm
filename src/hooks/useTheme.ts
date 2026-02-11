import { useState, useEffect } from 'react';
import { getTheme, setTheme as setThemeUtil, type Theme } from '../lib/utils';

export function useTheme() {
  const [theme, setThemeState] = useState<Theme>(getTheme());

  useEffect(() => {
    // Listen for theme changes from other components
    const handleStorageChange = (e: StorageEvent) => {
      if (e.key === 'tterm-theme' && e.newValue) {
        setThemeState(e.newValue as Theme);
      }
    };

    window.addEventListener('storage', handleStorageChange);
    return () => window.removeEventListener('storage', handleStorageChange);
  }, []);

  const setTheme = (newTheme: Theme) => {
    setThemeUtil(newTheme);
    setThemeState(newTheme);
  };

  return { theme, setTheme };
}
