import { apiClient, createWebSocket } from './client';

export const authService = {
    login: async (credentials) => {
        return apiClient.post('/users/authenticate', credentials);
    },
    register: async (userData) => {
        return apiClient.post('/users/register', userData);
    },
    authorize: async () => {
        return apiClient.get('/users/authorize');
    },
};

export const regionService = {
    getRegions: async () => {
        return apiClient.get('/regions');
    },
    createRegion: async (regionData) => {
        return apiClient.post('/regions', regionData);
    },
    updateRegion: async (id, regionData) => {
        return apiClient.put(`/regions/${id}`, regionData);
    },
    deleteRegion: async (id) => {
        return apiClient.delete(`/regions/${id}`);
    },
};

export const sensorService = {
    getSensors: async () => {
        return apiClient.get('/sensors');
    },
    getSensorsByRegion: async (regionId) => {
        return apiClient.get(`/sensors/region/${regionId}`);
    },
    getSensorData: async (sensorId) => {
        return apiClient.get(`/sensors/data/${sensorId}`);
    },
    streamSensorData: (sensorId, callback) => {
        return createWebSocket(`/sensors/data/sse/${sensorId}`).onMessage(callback);
    },
};

export const windowService = {
    getWindows: async () => {
        return apiClient.get('/windows');
    },
    getWindowsByRegion: async (regionId) => {
        return apiClient.get(`/windows/region/${regionId}`);
    },
    updateWindow: async (id, windowData) => {
        return apiClient.put(`/windows/${id}`, windowData);
    },
};

export const settingService = {
    getSettings: async () => {
        return apiClient.get('/settings');
    },
    createSetting: async (settingData) => {
        return apiClient.post('/settings', settingData);
    },
    updateSetting: async (id, settingData) => {
        return apiClient.put(`/settings/${id}`, settingData);
    },
    deleteSetting: async (id) => {
        return apiClient.delete(`/settings/${id}`);
    },
};

export const controlService = {
    sendCommand: async (command) => {
        return apiClient.post(`/control/${command}`);
    },
}; 