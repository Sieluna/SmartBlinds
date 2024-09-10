import React, { useCallback, useState } from 'react';
import CheckBoxIcon from '@mui/icons-material/CheckBox';
import CheckBoxOutlineBlankIcon from '@mui/icons-material/CheckBoxOutlineBlank';
import { Box, IconButton, Paper, Tab, Tabs, Tooltip } from '@mui/material';
import { useLanguage } from '../i18n/index.js';

const TAB_CONFIG = {
  regions: { groups: ['sensors', 'windows', 'settings'] },
  sensors: { groups: [] },
  windows: { groups: ['debug', 'settings'] },
  settings: { groups: [] },
  debug: { groups: [] },
};

export function Dashboard() {
  const { t } = useLanguage();
  const [activeTabs, setActiveTabs] = useState(new Set(['regions']));
  const [multiSelect, setMultiSelect] = useState(false);

  const handleTabChange = useCallback(
    (event, newValue) => {
      if (multiSelect) {
        const newTabs = new Set(activeTabs);
        if (newTabs.has(newValue)) {
          newTabs.delete(newValue);
        } else {
          newTabs.add(newValue);
        }
        setActiveTabs(newTabs);
      } else {
        setActiveTabs(new Set([newValue]));
      }
    },
    [activeTabs, multiSelect]
  );

  const toggleMultiSelect = useCallback(() => {
    setMultiSelect(prev => !prev);
    if (!multiSelect) {
      setActiveTabs(new Set(['regions']));
    }
  }, [multiSelect]);

  const isTabActive = useCallback(
    tab => {
      return activeTabs.has(tab);
    },
    [activeTabs]
  );

  return (
    <Box sx={{ width: '100%' }}>
      <Paper sx={{ width: '100%', mb: 2, display: 'flex', alignItems: 'center' }}>
        <Tabs
          value={Array.from(activeTabs)[0] || false}
          onChange={handleTabChange}
          indicatorColor="primary"
          textColor="primary"
          variant="scrollable"
          scrollButtons="auto"
          sx={{ flexGrow: 1 }}
        >
          {Object.keys(TAB_CONFIG).map(tab => (
            <Tab
              key={tab}
              label={t(`dashboard.${tab}`)}
              value={tab}
              sx={{
                opacity: isTabActive(tab) ? 1 : 0.5,
                fontWeight: isTabActive(tab) ? 'bold' : 'normal',
              }}
            />
          ))}
        </Tabs>
        <Tooltip title={t('dashboard.multiSelect')}>
          <IconButton onClick={toggleMultiSelect} color="primary">
            {multiSelect ? <CheckBoxIcon /> : <CheckBoxOutlineBlankIcon />}
          </IconButton>
        </Tooltip>
      </Paper>

      <Box sx={{ mt: 2 }}>
        {Object.keys(TAB_CONFIG).map(tab => (
          <Box
            key={tab}
            sx={{
              display: isTabActive(tab) ? 'block' : 'none',
              mb: 2,
            }}
          >
            {t(`dashboard.${tab}Content`)}
          </Box>
        ))}
      </Box>
    </Box>
  );
}
