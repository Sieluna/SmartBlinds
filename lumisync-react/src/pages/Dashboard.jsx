import React, { useEffect, useState } from 'react';
import SettingsIcon from '@mui/icons-material/Settings';
import SyncIcon from '@mui/icons-material/Sync';
import ThermostatIcon from '@mui/icons-material/Thermostat';
import WbSunnyIcon from '@mui/icons-material/WbSunny';
import Alert from '@mui/material/Alert';
import Box from '@mui/material/Box';
import CircularProgress from '@mui/material/CircularProgress';
import Grid from '@mui/material/Grid';
import Typography from '@mui/material/Typography';
import {
  useRegionService,
  useSensorService,
  useSettingService,
  useWindowService,
} from '../api/index.js';
import { ActivityList, ArenaChart, StatusCard, WindowStatusCards } from '../components/index.js';
import { useLanguage } from '../i18n/index.js';

const statusConfig = {
  regions: SettingsIcon,
  sensors: ThermostatIcon,
  windows: WbSunnyIcon,
  schedules: SyncIcon,
};

export function Dashboard() {
  const { t } = useLanguage();
  const [loading, setLoading] = useState(true);
  const [errors, setErrors] = useState({});
  const [dashboardData, setDashboardData] = useState({
    stats: {
      regions: { value: 0, change: '+0', trendDirection: 'up' },
      sensors: { value: 0, change: '+0', trendDirection: 'up' },
      windows: { value: 0, change: '+0', trendDirection: 'up' },
      schedules: { value: 0, change: '+0', trendDirection: 'up' },
    },
    environmentData: [],
    activities: [],
    windows: [],
  });

  const regionService = useRegionService();
  const sensorService = useSensorService();
  const windowService = useWindowService();
  const settingService = useSettingService();

  useEffect(() => {
    const fetchData = async () => {
      try {
        setLoading(true);
        setErrors({});

        const [regionsResult, sensorsResult, windowsResult, settingsResult] =
          await Promise.allSettled([
            regionService.getRegions(),
            sensorService.getSensors(),
            windowService.getWindows(),
            settingService.getSettings(),
          ]);

        let regions = [];
        if (regionsResult.status === 'fulfilled') {
          regions = regionsResult.value;
        } else {
          setErrors(prev => ({ ...prev, regions: 'Load region data fail' }));
        }

        let sensors = [];
        let sensorData = [];
        if (sensorsResult.status === 'fulfilled') {
          sensors = sensorsResult.value;
          try {
            sensorData = await Promise.all(
              sensors.map(sensor => sensorService.getSensorData(sensor.id))
            );
          } catch (error) {
            setErrors(prev => ({ ...prev, sensorData: 'Load sensor data fail' }));
          }
        } else {
          setErrors(prev => ({ ...prev, sensors: 'Load sensor data fail' }));
        }

        let windows = [];
        if (windowsResult.status === 'fulfilled') {
          windows = windowsResult.value;
        } else {
          setErrors(prev => ({ ...prev, windows: 'Load setting data fail' }));
        }

        let settings = [];
        if (settingsResult.status === 'fulfilled') {
          settings = settingsResult.value;
        } else {
          setErrors(prev => ({ ...prev, settings: 'Load setting data fail' }));
        }

        const environmentData = sensorData.flat().map(data => ({
          time: new Date(data.timestamp).toLocaleTimeString(),
          temperature: data.temperature,
          light: data.light,
        }));

        const activities = [
          ...windows.map(window => ({
            id: window.id,
            title: t('dashboard.activities.windowUpdate'),
            description: `${window.name} state updated`,
            time: new Date().toLocaleTimeString(),
          })),
          ...sensorData.flat().map(data => ({
            id: data.sensor_id,
            title: t('dashboard.activities.sensorUpdate'),
            description: `Sensor ${data.sensor_id} Updated`,
            time: new Date(data.timestamp).toLocaleTimeString(),
          })),
        ];

        setDashboardData({
          stats: {
            regions: {
              value: regions.length,
              change: regions.length > 0 ? `+${regions.length}` : '0',
              trendDirection: 'up',
            },
            sensors: {
              value: sensors.length,
              change: sensors.length > 0 ? `+${sensors.length}` : '0',
              trendDirection: 'up',
            },
            windows: {
              value: windows.length,
              change: windows.length > 0 ? `+${windows.length}` : '0',
              trendDirection: 'up',
            },
            schedules: {
              value: settings.length,
              change: settings.length > 0 ? `+${settings.length}` : '0',
              trendDirection: 'up',
            },
          },
          environmentData,
          activities,
          windows: windows.map(window => ({
            id: window.id,
            name: window.name,
            state: window.state,
            light: window.light || 0,
            temp: window.temperature || 0,
          })),
        });
      } catch (error) {
        console.error('Load dashboard data fail:', error);
        setErrors(prev => ({ ...prev, general: 'Load dashboard data fail' }));
      } finally {
        setLoading(false);
      }
    };

    fetchData();
  }, [t]);

  const renderError = errorKey => {
    if (errors[errorKey]) {
      return (
        <Typography color="error" variant="body2">
          {errors[errorKey]}
        </Typography>
      );
    }
    return null;
  };

  if (loading) {
    return (
      <Box display="flex" justifyContent="center" alignItems="center" height="100vh">
        <CircularProgress />
      </Box>
    );
  }

  return (
    <Grid container spacing={3} maxWidth="xl" direction="column">
      {errors.general && (
        <Grid item xs={12}>
          <Alert severity="error">{errors.general}</Alert>
        </Grid>
      )}

      <Typography variant="h4" gutterBottom>
        {t('dashboard.title')}
      </Typography>
      <Typography variant="body1" color="text.secondary">
        {t('dashboard.subtitle')}
      </Typography>

      <Grid container spacing={3}>
        {Object.entries(statusConfig).map(([key, Icon]) => (
          <Grid item key={key} size={{ xs: 12, sm: 3 }}>
            <StatusCard
              title={t(`dashboard.stats.${key}`)}
              value={dashboardData.stats[key].value}
              change={dashboardData.stats[key].change}
              trendDirection={dashboardData.stats[key].trendDirection}
              icon={Icon}
            />
            {renderError(key)}
          </Grid>
        ))}
      </Grid>

      <Grid container spacing={3}>
        <Grid item size={{ xs: 12, md: 7 }}>
          <Box
            sx={{
              height: '100%',
              display: 'flex',
              flexDirection: 'column',
            }}
          >
            <ArenaChart
              data={dashboardData.environmentData}
              title={t('dashboard.environment.title')}
              temperatureLabel={t('dashboard.environment.temperature')}
              lightLabel={t('dashboard.environment.light')}
            />
          </Box>
        </Grid>
        <Grid item size={{ xs: 12, md: 5 }}>
          <Box
            sx={{
              height: '100%',
              display: 'flex',
              flexDirection: 'column',
            }}
          >
            <ActivityList
              activities={dashboardData.activities}
              title={t('dashboard.activities.title')}
            />
          </Box>
        </Grid>
      </Grid>

      <Grid container spacing={3}>
        <Box sx={{ width: '100%' }}>
          <WindowStatusCards
            windows={dashboardData.windows}
            title={t('dashboard.windows.title')}
            stateLabels={{
              open: t('dashboard.windows.open'),
              closed: t('dashboard.windows.closed'),
              opening: t('dashboard.windows.opening'),
              closing: t('dashboard.windows.closing'),
              error: t('dashboard.windows.error'),
            }}
          />
        </Box>
      </Grid>
    </Grid>
  );
}
