import {
  createContext,
  createSignal,
  createMemo,
  createEffect,
  useContext,
  onMount,
  onCleanup,
} from 'solid-js';

/**
 * Available themes for the application
 * @type {Object}
 */
const THEMES = {
  LIGHT: 'light',
  DARK: 'dark',
  SYSTEM: 'system',
};

/**
 * Theme context for providing theme state throughout the application
 */
const ThemeContext = createContext();

/**
 * Get initial theme from localStorage or use system default
 * @returns {string} The initial theme
 */
function getInitialTheme() {
  const savedTheme = localStorage.getItem('theme');
  if (savedTheme && Object.values(THEMES).includes(savedTheme)) {
    return savedTheme;
  }
  return THEMES.SYSTEM;
}

/**
 * Provider component for theme functionality
 * @param {Object} props - Component props
 * @param {any} props.children - Child components
 */
export function ThemeProvider(props) {
  const [theme, setTheme] = createSignal(getInitialTheme());

  // Determine if the theme is dark
  const isDark = createMemo(
    () =>
      theme() === THEMES.DARK ||
      (theme() === THEMES.SYSTEM && window.matchMedia('(prefers-color-scheme: dark)').matches)
  );

  // Apply theme to document
  const applyTheme = () => {
    document.documentElement.setAttribute('data-theme', isDark() ? 'dark' : 'light');
  };

  // Apply theme immediately on mount to prevent flashing
  onMount(() => {
    applyTheme();

    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    const syncSystem = () => theme() === THEMES.SYSTEM && applyTheme();

    mediaQuery.addEventListener('change', syncSystem);

    onCleanup(() => {
      mediaQuery.removeEventListener('change', syncSystem);
    });
  });

  // Watch for theme changes
  createEffect(() => {
    const currentTheme = theme();
    localStorage.setItem('theme', currentTheme);
    applyTheme(currentTheme);
  });

  const toggleTheme = () => {
    setTheme((prev) =>
      prev === THEMES.LIGHT
        ? THEMES.DARK
        : prev === THEMES.DARK
          ? THEMES.LIGHT
          : isDark()
            ? THEMES.LIGHT
            : THEMES.DARK
    );
  };

  return (
    <ThemeContext.Provider
      value={{
        theme,
        isDark,
        themes: THEMES,
        setTheme,
        toggleTheme,
      }}
    >
      {props.children}
    </ThemeContext.Provider>
  );
}

/**
 * Custom hook for using theme context
 * @returns {Object} Theme context value
 */
export function useTheme() {
  const context = useContext(ThemeContext);
  if (!context) {
    throw new Error('useTheme must be used within a ThemeProvider');
  }
  return context;
}
