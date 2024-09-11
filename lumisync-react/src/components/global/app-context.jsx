import React, { createContext, useContext, useEffect, useReducer } from 'react';

const initialState = {
  currentComponent: 'Dashboard',
  theme: localStorage.getItem('theme') || 'light',
  language: 'en',
};

const AppContext = createContext();

function appReducer(state, action) {
  switch (action.type) {
    case 'SET_COMPONENT':
      return { ...state, currentComponent: action.payload };
    case 'SET_THEME':
      localStorage.setItem('theme', action.payload);
      return { ...state, theme: action.payload };
    case 'SET_LANGUAGE':
      return { ...state, language: action.payload };
    default:
      return state;
  }
}

export function AppProvider({ children }) {
  const [state, dispatch] = useReducer(appReducer, initialState);

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', state.theme);
  }, [state.theme]);

  const setComponent = component => {
    dispatch({ type: 'SET_COMPONENT', payload: component });
  };

  const setTheme = theme => {
    dispatch({ type: 'SET_THEME', payload: theme });
  };

  const setLanguage = language => {
    dispatch({ type: 'SET_LANGUAGE', payload: language });
  };

  const value = {
    state,
    actions: {
      setComponent,
      setTheme,
      setLanguage,
    },
  };

  return <AppContext.Provider value={value}>{children}</AppContext.Provider>;
}

export function useApp() {
  const context = useContext(AppContext);
  if (!context) {
    throw new Error('useApp must be used within an AppProvider');
  }
  return context;
}
