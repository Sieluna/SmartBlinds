import React, { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import CloseIcon from '@mui/icons-material/Close';
import {
  Alert,
  Box,
  Button,
  Dialog,
  DialogContent,
  DialogTitle,
  Divider,
  IconButton,
  Tab,
  Tabs,
  TextField,
  Typography,
} from '@mui/material';
import { useAuthService } from '../api';
import { useLanguage } from '../i18n/LanguageContext';

export function Login() {
  const navigate = useNavigate();
  const { t } = useLanguage();
  const authService = useAuthService();
  const [activeTab, setActiveTab] = useState(0);
  const [formData, setFormData] = useState({
    email: '',
    password: '',
    group: '',
  });
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  const handleTabChange = (event, newValue) => {
    setActiveTab(newValue);
    setError('');
  };

  const handleChange = e => {
    const { name, value } = e.target;
    setFormData(prev => ({
      ...prev,
      [name]: value,
    }));
  };

  const handleSubmit = async e => {
    e.preventDefault();
    setLoading(true);
    setError('');

    try {
      if (activeTab === 0) {
        // Login
        await authService.login({
          email: formData.email,
          password: formData.password,
        });
      } else {
        // Register
        await authService.register({
          email: formData.email,
          password: formData.password,
          group: formData.group,
        });
      }
      navigate('/');
    } catch (err) {
      console.error('Auth error:', err);
      setError(err.message || (activeTab === 0 ? t('login.loginError') : t('login.registerError')));
    } finally {
      setLoading(false);
    }
  };

  const renderFormFields = () => (
    <>
      {activeTab === 1 && (
        <TextField
          fullWidth
          label={t('login.group')}
          name="group"
          value={formData.group}
          onChange={handleChange}
          margin="normal"
          required
          variant="outlined"
          sx={{ mb: 2 }}
        />
      )}
      <TextField
        fullWidth
        label={t('login.email')}
        name="email"
        type="email"
        value={formData.email}
        onChange={handleChange}
        margin="normal"
        required
        variant="outlined"
        sx={{ mb: 2 }}
      />
      <TextField
        fullWidth
        label={t('login.password')}
        name="password"
        type="password"
        value={formData.password}
        onChange={handleChange}
        margin="normal"
        required
        variant="outlined"
        sx={{ mb: 2 }}
      />
    </>
  );

  return (
    <Dialog
      open={true}
      maxWidth="xs"
      fullWidth
      slotProps={{
        paper: {
          sx: {
            borderRadius: 2,
            boxShadow: '0 8px 32px rgba(0, 0, 0, 0.1)',
          },
        },
      }}
    >
      <DialogTitle sx={{ m: 0, p: 2 }}>
        <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
          <Typography variant="h6" component="div">
            {t('login.title')}
          </Typography>
          <IconButton
            aria-label={t('common.close')}
            onClick={() => navigate('/')}
            sx={{
              color: theme => theme.palette.grey[500],
            }}
          >
            <CloseIcon />
          </IconButton>
        </Box>
      </DialogTitle>
      <Divider />
      <DialogContent sx={{ p: 3 }}>
        <Tabs value={activeTab} onChange={handleTabChange} centered sx={{ mb: 3 }}>
          <Tab label={t('login.login')} />
          <Tab label={t('login.register')} />
        </Tabs>
        <form onSubmit={handleSubmit}>
          {renderFormFields()}
          {error && (
            <Alert severity="error" sx={{ mb: 2 }}>
              {error}
            </Alert>
          )}
          <Button
            type="submit"
            fullWidth
            variant="contained"
            size="large"
            disabled={loading}
            sx={{
              mt: 2,
              py: 1.5,
              borderRadius: 1,
              textTransform: 'none',
              fontSize: '1rem',
            }}
          >
            {loading
              ? t('common.loading')
              : activeTab === 0
                ? t('login.login')
                : t('login.register')}
          </Button>
        </form>
      </DialogContent>
    </Dialog>
  );
}
