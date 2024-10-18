import { createContext, createSignal, useContext, createEffect } from 'solid-js';

/**
 * Available languages for the application
 * @type {Object}
 */
const LANGUAGES = {
  EN: 'en',
  ZH: 'zh',
};

/**
 * Language context for providing i18n functionality
 */
const LanguageContext = createContext();

/**
 * Get initial language from localStorage or browser setting
 * @returns {string} The initial language
 */
function getInitialLang() {
  const savedLang = localStorage.getItem('language');
  if (savedLang && Object.values(LANGUAGES).includes(savedLang)) {
    return savedLang;
  }

  const browserLang = navigator.language.split('-')[0];
  if (Object.values(LANGUAGES).includes(browserLang)) {
    return browserLang;
  }

  return LANGUAGES.EN;
}

/**
 * Provider component for language functionality
 * @param {Object} props - Component props
 * @param {any} props.children - Child components
 * @param {Object} props.translations - Translation dictionaries for all supported languages
 */
export function LanguageProvider(props) {
  const [language, setLanguage] = createSignal(getInitialLang());
  const [translations, setTranslations] = createSignal({});

  createEffect(() => {
    if (props.translations) {
      setTranslations(props.translations);
    }
  });

  createEffect(() => {
    if (typeof document !== 'undefined') {
      document.documentElement.lang = language();
    }
  });

  /**
   * Translate a key to the current language
   * @param {string} key - Translation key
   * @param {Object} params - Replacement parameters
   * @returns {string} Translated text
   */
  const t = (key, params = {}) => {
    const keys = key.split('.');
    let value = translations()[language()] || {};

    // Traverse the nested keys
    for (const k of keys) {
      value = value?.[k];
      if (value === undefined) break;
    }

    if (typeof value !== 'string') {
      // Fallback to English or return the key itself
      let fallback = translations()[LANGUAGES.EN];
      for (const k of keys) {
        fallback = fallback?.[k];
        if (fallback === undefined) break;
      }
      value = typeof fallback === 'string' ? fallback : key;
    }

    // Replace parameters in the string
    return value.replace(/\{\{(\w+)\}\}/g, (_, key) =>
      Object.prototype.hasOwnProperty.call(params, key) ? params[key] : `{{${key}}}`
    );
  };

  /**
   * Change the current language
   * @param {string} newLang - New language code
   */
  const changeLanguage = (newLang) => {
    if (!Object.values(LANGUAGES).includes(newLang)) {
      console.warn(`Language ${newLang} is not supported`);
      return;
    }

    localStorage.setItem('language', newLang);
    setLanguage(newLang);
    document.documentElement.lang = newLang;
  };

  /**
   * Add or update translations
   * @param {Object} newTranslations - New translations to add
   */
  const addTranslations = (newTranslations) => {
    setTranslations((prev) => ({ ...prev, ...newTranslations }));
  };

  return (
    <LanguageContext.Provider
      value={{
        language,
        languages: LANGUAGES,
        t,
        changeLanguage,
        addTranslations,
      }}
    >
      {props.children}
    </LanguageContext.Provider>
  );
}

/**
 * Custom hook for using translations
 * @returns {Object} Language context value
 */
export function useTranslation() {
  const context = useContext(LanguageContext);
  if (!context) {
    throw new Error('useTranslation must be used within a LanguageProvider');
  }
  return context;
}
