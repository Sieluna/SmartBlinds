import { createContext, createSignal, createEffect, useContext, onMount, onCleanup } from 'solid-js';
import { createStore } from 'solid-js/store';

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
 * Provider component for theme functionality
 * @param {Object} props - Component props
 * @param {any} props.children - Child components
 */
export function ThemeProvider(props) {
  // Get initial theme from localStorage or use system default
  const getInitialTheme = () => {
    const savedTheme = localStorage.getItem('theme');
    if (savedTheme && Object.values(THEMES).includes(savedTheme)) {
      return savedTheme;
    }
    return THEMES.SYSTEM;
  };

  const [theme, setTheme] = createSignal(getInitialTheme());
  const [themeState, setThemeState] = createStore({
    isDark: false,
    currentTheme: getInitialTheme(),
  });

  // Apply theme to document
  const applyTheme = newTheme => {
    const root = document.documentElement;
    const isDark =
      newTheme === THEMES.DARK ||
      (newTheme === THEMES.SYSTEM && window.matchMedia('(prefers-color-scheme: dark)').matches);

    setThemeState({
      isDark,
      currentTheme: newTheme,
    });

    if (newTheme === THEMES.SYSTEM) {
      root.setAttribute('data-theme', isDark ? 'dark' : 'light');
    } else {
      root.setAttribute('data-theme', newTheme);
    }

    localStorage.setItem('theme', newTheme);
  };

  // Apply theme immediately on mount to prevent flashing
  onMount(() => {
    applyTheme(theme());

    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    const handleChange = () => {
      if (theme() === THEMES.SYSTEM) {
        applyTheme(THEMES.SYSTEM);
      }
    };

    mediaQuery.addEventListener('change', handleChange);

    onCleanup(() => {
      mediaQuery.removeEventListener('change', handleChange);
    });
  });

  // Watch for theme changes
  createEffect(() => {
    const currentTheme = theme();
    applyTheme(currentTheme);
  });

  const toggleTheme = () => {
    const currentTheme = theme();
    if (currentTheme === THEMES.LIGHT) {
      setTheme(THEMES.DARK);
    } else if (currentTheme === THEMES.DARK) {
      setTheme(THEMES.LIGHT);
    } else {
      setTheme(themeState.isDark ? THEMES.LIGHT : THEMES.DARK);
    }
  };

  return (
    <ThemeContext.Provider
      value={{
        theme: () => theme(),
        isDark: () => themeState.isDark,
        themes: THEMES,
        setTheme: (newTheme) => setTheme(newTheme),
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
