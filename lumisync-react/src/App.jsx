import React from 'react';
import { Navigate, Route, BrowserRouter as Router, Routes } from 'react-router-dom';
import CssBaseline from '@mui/material/CssBaseline';
import { ThemeProvider, createTheme } from '@mui/material/styles';
import { ApiProvider, useApi } from './api/index.js';
import { AppProvider, AppShell, Auth, useApp } from './components/index.js';
import { LanguageProvider } from './i18n/index.js';
import { Dashboard } from './pages/index.js';

function AppContent() {
  const { state: apiState } = useApi();
  const { state: appState } = useApp();
  const isAuthenticated = !!apiState.auth.token;

  const theme = createTheme({
    palette: {
      mode: appState.theme,
    },
  });

  return (
    <ThemeProvider theme={theme}>
      <Router>
        <AppShell>
          <Routes>
            <Route path="/" element={<Navigate to="/dashboard" replace />} />
            <Route path="/dashboard" element={<Dashboard />} />
          </Routes>
        </AppShell>
        {!isAuthenticated && <Auth />}
      </Router>
    </ThemeProvider>
  );
}

function App() {
  return (
    <AppProvider>
      <ApiProvider>
        <LanguageProvider>
          <CssBaseline />
          <AppContent />
        </LanguageProvider>
      </ApiProvider>
    </AppProvider>
  );
}

export default App;
