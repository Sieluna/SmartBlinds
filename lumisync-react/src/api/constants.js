// Base URL for API endpoints
export const API_BASE_URL = `${globalThis.__APP_API_URL__}`;

// WebSocket base URL
export const WS_BASE_URL = API_BASE_URL.replace('http', 'ws');

export const STORAGE_KEY = 'auth_token';

export class ApiError extends Error {
  constructor(message, status) {
    super(message);
    this.status = status;
    this.name = 'ApiError';
  }
}
