import ThermostatIcon from '@mui/icons-material/Thermostat';
import WbSunnyIcon from '@mui/icons-material/WbSunny';
import Box from '@mui/material/Box';
import Card from '@mui/material/Card';
import CardContent from '@mui/material/CardContent';
import Chip from '@mui/material/Chip';
import Grid from '@mui/material/Grid';
import Typography from '@mui/material/Typography';

export function WindowStatus({ windows, title, stateLabels }) {
  const getWindowStatus = state => {
    if (state > 0) return { label: stateLabels.open, color: 'success' };
    return { label: stateLabels.closed, color: 'warning' };
  };

  return (
    <Card sx={{ height: '100%' }}>
      <CardContent>
        <Typography variant="h6" gutterBottom>
          {title}
        </Typography>
        <Grid container spacing={2}>
          {windows.map(window => {
            const status = getWindowStatus(window.state);
            return (
              <Grid item xs={12} sm={6} key={window.id}>
                <Card variant="outlined">
                  <CardContent>
                    <Box
                      sx={{
                        display: 'flex',
                        justifyContent: 'space-between',
                        alignItems: 'center',
                        mb: 1,
                      }}
                    >
                      <Typography variant="subtitle1">{window.name}</Typography>
                      <Chip label={status.label} color={status.color} size="small" />
                    </Box>
                    <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
                      <WbSunnyIcon color="warning" fontSize="small" />
                      <Typography variant="body2">{window.light} lux</Typography>
                    </Box>
                    <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
                      <ThermostatIcon color="error" fontSize="small" />
                      <Typography variant="body2">{window.temp}°C</Typography>
                    </Box>
                  </CardContent>
                </Card>
              </Grid>
            );
          })}
        </Grid>
      </CardContent>
    </Card>
  );
}
