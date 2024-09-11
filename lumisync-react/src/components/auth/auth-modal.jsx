import React, { useState } from 'react';
import CloseIcon from '@mui/icons-material/Close';
import Alert from '@mui/material/Alert';
import Box from '@mui/material/Box';
import Button from '@mui/material/Button';
import Dialog from '@mui/material/Dialog';
import DialogContent from '@mui/material/DialogContent';
import DialogTitle from '@mui/material/DialogTitle';
import Divider from '@mui/material/Divider';
import IconButton from '@mui/material/IconButton';
import Tab from '@mui/material/Tab';
import Tabs from '@mui/material/Tabs';
import TextField from '@mui/material/TextField';
import Typography from '@mui/material/Typography';
import { useAuthService } from '../../api/index.js';
import { useLanguage } from '../../i18n/index.js';
import { useApp } from '../global/app-context.jsx';

export function Auth() {
  const { t } = useLanguage();
  const authService = useAuthService();
  const { actions } = useApp();
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
      actions.setComponent('Dashboard');
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
    <Dialog open={true} maxWidth="xs" fullWidth>
      <DialogTitle>
        <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
          <Typography variant="h6" component="div">
            {t('login.title')}
          </Typography>
          <IconButton
            aria-label={t('common.close')}
            onClick={() => actions.setComponent('Dashboard')}
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
