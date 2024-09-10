import { Route, BrowserRouter as Router, Routes } from 'react-router-dom';
import LogoutIcon from '@mui/icons-material/Logout';
import { ThemeProvider, createTheme } from '@mui/material';
import { AppBar, Box, Grid, IconButton, Toolbar, Typography } from '@mui/material';
import CssBaseline from '@mui/material/CssBaseline';
import { ApiProvider, useApi, useAuthService } from './api';
import { TestApi } from './components/TestApi';
import { Login } from './components/index.js';
import { Dashboard } from './components/index.js';
import { LanguageProvider } from './i18n/index.js';

const theme = createTheme({
  palette: {
    mode: 'light',
    primary: {
      main: '#1976d2',
    },
    background: {
      default: '#f5f5f5',
    },
  },
  components: {
    MuiButton: {
      styleOverrides: {
        root: {
          textTransform: 'none',
        },
      },
    },
  },
});

const MainLayout = ({ children }) => {
  const { state } = useApi();
  const authService = useAuthService();

  const handleLogout = () => {
    if (authService) {
      authService.logout();
    }
  };

  return (
    <Box sx={{ display: 'flex', flexDirection: 'column', minHeight: '100vh' }}>
      <AppBar position="static" color="primary" elevation={0}>
        <Toolbar>
          <Typography variant="h6" component="div" sx={{ flexGrow: 1 }}>
            Smart Blinds
          </Typography>
          {state.auth.token && (
            <IconButton color="inherit" onClick={handleLogout}>
              <LogoutIcon />
            </IconButton>
          )}
        </Toolbar>
      </AppBar>
      <Box
        sx={{
          flexGrow: 1,
          background: 'linear-gradient(135deg, #f5f7fa 0%, #c3cfe2 100%)',
          p: 3,
        }}
      >
        {children}
      </Box>
    </Box>
  );
};

function Page() {
  const { state } = useApi();

  return (
    <Router>
      <MainLayout>
        <Routes>
          <Route path="/test" element={<TestApi />} />
          <Route
            path="/*"
            element={
              <Box>
                <Grid container spacing={3}>
                  <Grid item xs={12}>
                    {state.auth.token ? <Dashboard /> : null}
                  </Grid>
                </Grid>
              </Box>
            }
          />
        </Routes>
        {!state.auth.token && <Login />}
      </MainLayout>
    </Router>
  );
}

function App() {
  return (
    <ApiProvider>
      <ThemeProvider theme={theme}>
        <CssBaseline />
        <LanguageProvider>
          <Page />
        </LanguageProvider>
      </ThemeProvider>
    </ApiProvider>
  );
}

export default App;
