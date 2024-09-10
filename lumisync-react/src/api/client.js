const API_BASE_URL = import.meta.env.VITE_API_URL || 'http://localhost:3000';

class ApiError extends Error {
    constructor(message, status) {
        super(message);
        this.status = status;
    }
}

const handleResponse = async (response) => {
    if (!response.ok) {
        const error = await response.json().catch(() => ({ message: 'Request failed' }));
        throw new ApiError(error.message, response.status);
    }
    return response.json();
};

const getHeaders = (options = {}) => {
    const headers = {
        'Content-Type': 'application/json',
        ...options.headers,
    };

    const token = localStorage.getItem('token');
    if (token) {
        headers['Authorization'] = `Bearer ${token}`;
    }

    return headers;
};

export const apiClient = {
    async get(endpoint, options = {}) {
        try {
            useApiStore.getState().setLoading(true);
            const response = await fetch(`${API_BASE_URL}${endpoint}`, {
                method: 'GET',
                headers: getHeaders(options),
                ...options,
            });
            return await handleResponse(response);
        } catch (error) {
            useApiStore.getState().setError(error);
            throw error;
        } finally {
            useApiStore.getState().setLoading(false);
        }
    },

    async post(endpoint, data, options = {}) {
        try {
            useApiStore.getState().setLoading(true);
            const response = await fetch(`${API_BASE_URL}${endpoint}`, {
                method: 'POST',
                headers: getHeaders(options),
                body: JSON.stringify(data),
                ...options,
            });
            return await handleResponse(response);
        } catch (error) {
            useApiStore.getState().setError(error);
            throw error;
        } finally {
            useApiStore.getState().setLoading(false);
        }
    },

    async put(endpoint, data, options = {}) {
        try {
            useApiStore.getState().setLoading(true);
            const response = await fetch(`${API_BASE_URL}${endpoint}`, {
                method: 'PUT',
                headers: getHeaders(options),
                body: JSON.stringify(data),
                ...options,
            });
            return await handleResponse(response);
        } catch (error) {
            useApiStore.getState().setError(error);
            throw error;
        } finally {
            useApiStore.getState().setLoading(false);
        }
    },

    async delete(endpoint, options = {}) {
        try {
            useApiStore.getState().setLoading(true);
            const response = await fetch(`${API_BASE_URL}${endpoint}`, {
                method: 'DELETE',
                headers: getHeaders(options),
                ...options,
            });
            return await handleResponse(response);
        } catch (error) {
            useApiStore.getState().setError(error);
            throw error;
        } finally {
            useApiStore.getState().setLoading(false);
        }
    },
};

export const createWebSocket = (endpoint) => {
    const token = localStorage.getItem('token');
    const ws = new WebSocket(`${API_BASE_URL.replace('http', 'ws')}${endpoint}?token=${token}`);
    
    return {
        ws,
        onMessage: (callback) => {
            ws.onmessage = (event) => {
                try {
                    const data = JSON.parse(event.data);
                    callback(data);
                } catch (error) {
                    console.error('WebSocket message parsing error:', error);
                }
            };
        },
        onError: (callback) => {
            ws.onerror = (error) => callback(error);
        },
        onClose: (callback) => {
            ws.onclose = () => callback();
        },
        close: () => ws.close(),
    };
};
