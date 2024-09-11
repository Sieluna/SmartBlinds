import React from 'react';
import { Box, Card, CardContent, Typography } from '@mui/material';
import { useTheme } from '@mui/material/styles';

export function StatusCard({ title, value, change, trendDirection, icon: Icon }) {
  const theme = useTheme();
  const isPositive = trendDirection === 'up';
  const color = isPositive ? theme.palette.success.main : theme.palette.error.main;

  return (
    <Card>
      <CardContent>
        <Box display="flex" justifyContent="space-between" alignItems="center">
          <Box>
            <Typography color="text.secondary" gutterBottom>
              {title}
            </Typography>
            <Typography variant="h4" component="div">
              {value}
            </Typography>
            <Box display="flex" alignItems="center" mt={1}>
              <Typography variant="body2" sx={{ color, display: 'flex', alignItems: 'center' }}>
                {change}
              </Typography>
            </Box>
          </Box>
          <Box
            sx={{
              backgroundColor: theme.palette.primary.light,
              borderRadius: '50%',
              p: 1,
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
            }}
          >
            <Icon sx={{ color: theme.palette.primary.main }} />
          </Box>
        </Box>
      </CardContent>
    </Card>
  );
}
