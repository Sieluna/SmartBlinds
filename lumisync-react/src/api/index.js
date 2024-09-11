import { STORAGE_KEY } from './constants.js';
import { useApi } from './context.jsx';

export * from './context.jsx';
export * from './constants.js';

// Authentication Service
export const useAuthService = () => {
  const { dispatch, httpClient } = useApi();

  return {
    /**
     * Authenticate user, call `user_handle::authenticate_user`.
     *
     * @param {{email: string, password: string}} credentials - User credentials
     * @returns {Promise<string>} Authentication token
     */
    login: async credentials => {
      const token = await httpClient.post('/users/authenticate', credentials);
      localStorage.setItem(STORAGE_KEY, token);
      dispatch({ type: 'AUTH/SET_TOKEN', payload: token });
      return token;
    },

    /**
     * Register user, call `user_handle::create_user`.
     *
     * @param {{group: string, email: string, password: string, role: string}} userData - User data
     * @returns {Promise<string>} Authentication token
     */
    register: async userData => {
      const token = await httpClient.post('/users/register', userData);
      localStorage.setItem(STORAGE_KEY, token);
      dispatch({ type: 'AUTH/SET_TOKEN', payload: token });
      return token;
    },

    /**
     * Authorize user, call `user_handle::authorize_user`.
     *
     * @returns {Promise<string>} Authentication token
     */
    authorize: async () => {
      const token = await httpClient.get('/users/authorize');
      localStorage.setItem(STORAGE_KEY, token);
      dispatch({ type: 'AUTH/SET_TOKEN', payload: token });
      return token;
    },

    /**
     * User logout
     */
    logout: async () => {
      localStorage.removeItem(STORAGE_KEY);
      dispatch({ type: 'AUTH/CLEAR' });
    },
  };
};

/**
 * @typedef {Object} Region
 * @property {number} id - Region ID
 * @property {number} group_id - Group ID that owns the region
 * @property {string} name - Region name
 * @property {number} light - Average light in region
 * @property {number} temperature - Average temperature in region
 */

// Region Service
export const useRegionService = () => {
  const { state, dispatch, httpClient } = useApi();

  return {
    /**
     * Get all regions by current user, call `region_handle::get_regions`.
     *
     * @returns {Promise<Region[]>} List of regions
     */
    getRegions: async () => {
      const response = await httpClient.get('/regions');
      dispatch({ type: 'ENTITIES/UPDATE', payload: { regions: response } });
      return response;
    },

    /**
     * Create a region by current user, call `region_handle::create_region`.
     *
     * @param {{user_ids?: number[], name: string, light?: number, temperature?: number}} regionData - Region data
     * @returns {Promise<Region>} Newly created region
     */
    createRegion: async regionData => {
      const response = await httpClient.post('/regions', regionData);
      dispatch({
        type: 'ENTITIES/UPDATE',
        payload: { regions: [...state.entities.regions, response] },
      });
      return response;
    },

    /**
     * Update a region
     *
     * @param {number} id - Region ID
     * @param {Partial<Region>} regionData - Updated region data
     * @returns {Promise<Region>} Updated region
     */
    updateRegion: async (id, regionData) => {
      const response = await httpClient.put(`/regions/${id}`, regionData);
      dispatch({
        type: 'ENTITIES/UPDATE',
        payload: {
          regions: state.entities.regions.map(region => (region.id === id ? response : region)),
        },
      });
      return response;
    },

    /**
     * Delete a region
     *
     * @param {number} id - Region ID
     */
    deleteRegion: async id => {
      await httpClient.delete(`/regions/${id}`);
      dispatch({
        type: 'ENTITIES/UPDATE',
        payload: {
          regions: state.entities.regions.filter(region => region.id !== id),
        },
      });
    },
  };
};

/**
 * @typedef {Object} Sensor
 * @property {number} id - Sensor ID
 * @property {number} region_id - Region ID that owns the sensor
 * @property {string} name - Sensor name
 */

// Sensor Service
export const useSensorService = () => {
  const { state, dispatch, httpClient, wsManager } = useApi();

  return {
    /**
     * Get all sensors
     *
     * @returns {Promise<Sensor[]>} List of sensors
     */
    getSensors: async () => {
      const response = await httpClient.get('/sensors');
      dispatch({ type: 'ENTITIES/UPDATE', payload: { sensors: response } });
      return response;
    },

    /**
     * Get sensors by region
     *
     * @param {number} regionId - Region ID
     * @returns {Promise<Sensor[]>} List of sensors
     */
    getSensorsByRegion: async regionId => {
      const response = await httpClient.get(`/sensors/region/${regionId}`);
      dispatch({ type: 'ENTITIES/UPDATE', payload: { sensors: response } });
      return response;
    },

    /**
     * Get sensor data
     *
     * @param {number} sensorId - Sensor ID
     * @returns {Promise<SensorData[]>} List of sensor data
     */
    getSensorData: async sensorId => {
      const response = await httpClient.get(`/sensors/data/${sensorId}`);
      dispatch({
        type: 'ENTITIES/UPDATE',
        payload: {
          sensors: { ...state.entities.sensors, [sensorId]: response },
        },
      });
      return response;
    },

    /**
     * Stream sensor data
     *
     * @param {number} sensorId - Sensor ID
     * @param {function(SensorData): void} callback - Data callback function
     */
    streamSensorData: (sensorId, callback) => {
      wsManager.connect(`/sensors/data/sse/${sensorId}`, callback);
    },
  };
};

/**
 * @typedef {Object} Window
 * @property {number} id - Window ID
 * @property {number} region_id - Region ID that owns the window
 * @property {string} name - Window name
 * @property {number} state - Window state (light transmittance)
 */

// Window Service
export const useWindowService = () => {
  const { state, dispatch, httpClient } = useApi();

  return {
    /**
     * Get all windows
     *
     * @returns {Promise<Window[]>} List of windows
     */
    getWindows: async () => {
      const response = await httpClient.get('/windows');
      dispatch({ type: 'ENTITIES/UPDATE', payload: { windows: response } });
      return response;
    },

    /**
     * Get windows by region
     *
     * @param {number} regionId - Region ID
     * @returns {Promise<Window[]>} List of windows
     */
    getWindowsByRegion: async regionId => {
      const response = await httpClient.get(`/windows/region/${regionId}`);
      dispatch({ type: 'ENTITIES/UPDATE', payload: { windows: response } });
      return response;
    },

    /**
     * Update window state
     *
     * @param {number} id - Window ID
     * @param {Partial<Window>} windowData - Updated window data
     * @returns {Promise<Window>} Updated window
     */
    updateWindow: async (id, windowData) => {
      const response = await httpClient.put(`/windows/${id}`, windowData);
      dispatch({
        type: 'ENTITIES/UPDATE',
        payload: {
          windows: state.entities.windows.map(window => (window.id === id ? response : window)),
        },
      });
      return response;
    },
  };
};

/**
 * @typedef {Object} Setting
 * @property {number} id - Setting ID
 * @property {number} light - Expected light intensity
 * @property {number} temperature - Expected temperature
 * @property {Date} start - Setting start time
 * @property {Date} end - Setting end time
 */

// Setting Service
export const useSettingService = () => {
  const { state, dispatch, httpClient } = useApi();

  return {
    /**
     * Get all settings control by current user, call `setting_handle::get_setting`.
     *
     * @returns {Promise<Setting[]>} List of settings
     */
    getSettings: async () => {
      const response = await httpClient.get('/settings');
      dispatch({ type: 'ENTITIES/UPDATE', payload: { settings: response } });
      return response;
    },

    /**
     * Create a setting
     *
     * @param {Partial<Setting>} settingData - Setting data
     * @returns {Promise<Setting>} Newly created setting
     */
    createSetting: async settingData => {
      const response = await httpClient.post('/settings', settingData);
      dispatch({
        type: 'ENTITIES/UPDATE',
        payload: {
          settings: [...state.entities.settings, response],
        },
      });
      return response;
    },

    /**
     * Update a setting
     *
     * @param {number} id - Setting ID
     * @param {Partial<Setting>} settingData - Updated setting data
     * @returns {Promise<Setting>} Updated setting
     */
    updateSetting: async (id, settingData) => {
      const response = await httpClient.put(`/settings/${id}`, settingData);
      dispatch({
        type: 'ENTITIES/UPDATE',
        payload: {
          settings: state.entities.settings.map(setting =>
            setting.id === id ? response : setting
          ),
        },
      });
      return response;
    },

    /**
     * Delete a setting
     *
     * @param {number} id - Setting ID
     */
    deleteSetting: async id => {
      await httpClient.delete(`/settings/${id}`);
      dispatch({
        type: 'ENTITIES/UPDATE',
        payload: {
          settings: state.entities.settings.filter(setting => setting.id !== id),
        },
      });
    },
  };
};

// Control Service
export const useControlService = () => {
  const { httpClient } = useApi();

  return {
    /**
     * Send control command
     *
     * @param {string} command - Control command
     * @returns {Promise<void>}
     */
    sendCommand: async command => {
      return httpClient.post(`/control/${command}`);
    },
  };
};
