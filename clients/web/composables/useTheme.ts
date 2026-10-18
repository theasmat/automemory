import { ref, computed } from 'vue';

export type ThemeMode = 'system' | 'light' | 'dark';

const themeMode = ref<ThemeMode>('system');
const systemDark = ref(false);
let mediaQuery: MediaQueryList | null = null;
let initialized = false;

export function useTheme() {
  function handleSystemChange(e: MediaQueryListEvent) {
    systemDark.value = e.matches;
    if (themeMode.value === 'system') {
      applyTheme();
    }
  }

  function initTheme() {
    if (typeof window === 'undefined' || initialized) return;
    initialized = true;

    if (window.matchMedia) {
      mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
      systemDark.value = mediaQuery.matches;
      if (mediaQuery.addEventListener) {
        mediaQuery.addEventListener('change', handleSystemChange);
      } else if ((mediaQuery as any).addListener) {
        (mediaQuery as any).addListener(handleSystemChange);
      }
    }

    let stored: string | null = null;
    try {
      stored = localStorage.getItem('am_theme');
    } catch {
      // storage blocked
    }

    if (stored === 'light' || stored === 'dark' || stored === 'system') {
      themeMode.value = stored;
    } else {
      themeMode.value = 'system';
    }

    applyTheme();
  }

  const isDark = computed(() => {
    if (themeMode.value === 'dark') return true;
    if (themeMode.value === 'light') return false;
    return systemDark.value;
  });

  const themeLabel = computed(() => {
    if (themeMode.value === 'system') return 'System';
    return themeMode.value === 'dark' ? 'Dark' : 'Light';
  });

  function applyTheme() {
    if (typeof document === 'undefined') return;
    const root = document.documentElement;
    const dark = isDark.value;

    if (dark) {
      root.classList.add('dark');
      root.classList.remove('light');
      root.style.colorScheme = 'dark';
    } else {
      root.classList.add('light');
      root.classList.remove('dark');
      root.style.colorScheme = 'light';
    }
  }

  function setTheme(mode: ThemeMode) {
    themeMode.value = mode;
    try {
      localStorage.setItem('am_theme', mode);
    } catch {
      // storage blocked
    }
    applyTheme();
  }

  function cycleTheme() {
    // Cycles: System -> Light -> Dark -> System
    if (themeMode.value === 'system') {
      setTheme('light');
    } else if (themeMode.value === 'light') {
      setTheme('dark');
    } else {
      setTheme('system');
    }
  }

  function toggleTheme() {
    cycleTheme();
  }

  return {
    themeMode,
    isDark,
    themeLabel,
    initTheme,
    setTheme,
    cycleTheme,
    toggleTheme,
  };
}
