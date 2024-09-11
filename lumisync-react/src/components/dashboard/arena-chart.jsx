import React from 'react';
import Box from '@mui/material/Box';
import Card from '@mui/material/Card';
import CardContent from '@mui/material/CardContent';
import Typography from '@mui/material/Typography';
import { useTheme } from '@mui/material/styles';
import { LineChart } from '@mui/x-charts/LineChart';

export function ArenaChart({ data, title, temperatureLabel, lightLabel }) {
  const theme = useTheme();

  return (
    <Card sx={{ height: '100%' }}>
      <CardContent>
        <Typography variant="h6" gutterBottom>
          {title}
        </Typography>
        <Box sx={{ height: 300 }}>
          <LineChart
            dataset={data}
            series={[
              {
                dataKey: 'temperature',
                label: `${temperatureLabel} (°C)`,
                color: theme.palette.error.main,
              },
              {
                dataKey: 'light',
                label: `${lightLabel} (lux)`,
                color: theme.palette.warning.main,
              },
            ]}
            xAxis={[
              {
                dataKey: 'time',
                scaleType: 'point',
              },
            ]}
            yAxis={[
              {
                scaleType: 'linear',
                label: `${temperatureLabel} (°C)`,
              },
              {
                scaleType: 'linear',
                label: `${lightLabel} (lux)`,
                position: 'right',
              },
            ]}
            grid={{ vertical: true, horizontal: true }}
            margin={{ top: 20, right: 30, left: 20, bottom: 30 }}
          />
        </Box>
      </CardContent>
    </Card>
  );
}
