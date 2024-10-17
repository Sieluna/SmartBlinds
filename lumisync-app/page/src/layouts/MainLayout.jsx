import { LanguageProvider } from '../context/LanguageContext.jsx';
import { ThemeProvider } from '../context/ThemeContext.jsx';
import { translations } from '../locales/index.js';

/**
 * Main layout component that wraps the entire application
 * Provides theme and language context to all children
 * @param {Object} props - Component props
 */
export function MainLayout(props) {
  return (
    <ThemeProvider>
      <LanguageProvider translations={translations}>
        <div class="min-h-screen bg-gray-50 dark:bg-gray-900 text-gray-900 dark:text-gray-100 antialiased">
          {props.children}
        </div>
      </LanguageProvider>
    </ThemeProvider>
  );
}
