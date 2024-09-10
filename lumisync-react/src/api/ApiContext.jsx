import React, { createContext, useContext, useMemo, useReducer } from 'react';
import { STORAGE_KEY } from './constants.js';
import { HttpClient } from './http.js';
import { WebSocketManager } from './websocket.js';

const initialState = {
  auth: {
    token: localStorage.getItem(STORAGE_KEY) || null,
    user: null,
  },
  request: {
    loading: false,
    error: null,
  },
  entities: {
    regions: [],
    sensors: {},
    windows: [],
    settings: [],
  },
};

const apiReducer = (state, action) => {
  switch (action.type) {
    case 'AUTH/SET_TOKEN':
      return {
        ...state,
        auth: { ...state.auth, token: action.payload },
      };

    case 'AUTH/SET_USER':
      return {
        ...state,
        auth: { ...state.auth, user: action.payload },
      };

    case 'AUTH/CLEAR':
      return {
        ...state,
        auth: { token: null, user: null },
      };

    case 'REQUEST/START':
      return {
        ...state,
        request: { ...state.request, loading: true, error: null },
      };

    case 'REQUEST/END':
      return {
        ...state,
        request: { ...state.request, loading: false },
      };

    case 'REQUEST/ERROR':
      return {
        ...state,
        request: { ...state.request, loading: false, error: action.payload },
      };

    case 'ENTITIES/UPDATE':
      return {
        ...state,
        entities: { ...state.entities, ...action.payload },
      };

    default:
      return state;
  }
};

const ApiContext = createContext();

export function ApiProvider({ children }) {
  const [state, dispatch] = useReducer(apiReducer, initialState);

  const httpClient = useMemo(
    () => new HttpClient(() => state.auth.token, dispatch),
    [state.auth.token]
  );

  const wsManager = useMemo(() => new WebSocketManager(dispatch), []);

  const value = {
    state,
    dispatch,
    httpClient,
    wsManager,
  };

  return <ApiContext.Provider value={value}>{children}</ApiContext.Provider>;
}

export function useApi() {
  const context = useContext(ApiContext);
  if (!context) {
    throw new Error('useApi must be used within an ApiProvider');
  }
  return context;
}
