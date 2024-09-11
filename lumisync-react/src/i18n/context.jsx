import React, { createContext, useContext, useEffect, useState } from 'react';
import enUS from './locales/en-US.json';
import zhCN from './locales/zh-CN.json';

const LanguageContext = createContext();

const languages = {
  en: enUS,
  zh: zhCN,
};

const languageNames = {
  en: 'English',
  zh: '中文',
};

const languageMap = {
  zh: 'zh',
  'zh-CN': 'zh',
  'zh-TW': 'zh',
  'zh-HK': 'zh',
  en: 'en',
  'en-US': 'en',
  'en-GB': 'en',
  'en-CA': 'en',
  'en-AU': 'en',
};

function getBrowserLanguage() {
  const browserLanguages = navigator.languages || [navigator.language || navigator.userLanguage];

  for (const lang of browserLanguages) {
    const normalizedLang = lang.split('-')[0].toLowerCase();
    if (languageMap[normalizedLang]) {
      return languageMap[normalizedLang];
    }
  }

  return 'en';
}

function getCurrentLanguage() {
  const storedLanguage = localStorage.getItem('language');
  if (storedLanguage && languages[storedLanguage]) {
    return storedLanguage;
  }

  return getBrowserLanguage();
}

export function LanguageProvider({ children }) {
  const [language, setLanguage] = useState(getCurrentLanguage);

  useEffect(() => {
    localStorage.setItem('language', language);
  }, [language]);

  const t = (key, params = {}) => {
    const keys = key.split('.');
    let value = languages[language];
    for (const k of keys) {
      value = value?.[k];
    }

    if (!value) return key;

    return value.replace(/\{(\w+)\}/g, (match, param) => params[param] || match);
  };

  const changeLanguage = newLanguage => {
    if (languages[newLanguage]) {
      setLanguage(newLanguage);
    }
  };

  const getSupportedLanguages = () => {
    return Object.keys(languages).map(code => ({
      code,
      name: languageNames[code],
      isCurrent: code === language,
    }));
  };

  return (
    <LanguageContext.Provider
      value={{
        language,
        setLanguage: changeLanguage,
        t,
        supportedLanguages: getSupportedLanguages(),
      }}
    >
      {children}
    </LanguageContext.Provider>
  );
}

export function useLanguage() {
  const context = useContext(LanguageContext);
  if (!context) {
    throw new Error('useLanguage must be used within a LanguageProvider');
  }
  return context;
}
