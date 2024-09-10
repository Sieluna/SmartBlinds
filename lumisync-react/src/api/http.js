import { API_BASE_URL, ApiError } from './constants.js';

export class HttpClient {
  constructor(getToken, dispatch) {
    this.getToken = getToken;
    this.dispatch = dispatch;
    this.interceptors = {
      request: [],
      response: [],
    };
  }

  async request(endpoint, options = {}) {
    try {
      let processedOptions = options;
      for (const interceptor of this.interceptors.request) {
        processedOptions = await interceptor(processedOptions);
      }

      const headers = {
        'Content-Type': 'application/json',
        ...(this.getToken() ? { Authorization: `Bearer ${this.getToken()}` } : {}),
        ...processedOptions.headers,
      };

      this.dispatch({ type: 'REQUEST/START' });

      const response = await fetch(`${API_BASE_URL}${endpoint}`, {
        ...processedOptions,
        headers,
      });

      if (!response.ok) {
        const errorText = await response.text();
        throw new ApiError(errorText || 'Request failed', response.status);
      }

      let processedResponse = response;
      for (const interceptor of this.interceptors.response) {
        processedResponse = await interceptor(processedResponse);
      }

      // Check content type and handle accordingly
      const contentType = response.headers.get('content-type');
      if (contentType && contentType.includes('application/json')) {
        try {
          return await processedResponse.json();
        } catch {
          return await processedResponse.text();
        }
      } else {
        return await processedResponse.text();
      }
    } catch (error) {
      this.dispatch({
        type: 'REQUEST/ERROR',
        payload: error.message || 'Request failed',
      });
      throw error;
    } finally {
      this.dispatch({ type: 'REQUEST/END' });
    }
  }

  get(endpoint) {
    return this.request(endpoint, {
      method: 'GET',
    });
  }

  post(endpoint, data) {
    return this.request(endpoint, {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }

  put(endpoint, data) {
    return this.request(endpoint, {
      method: 'PUT',
      body: JSON.stringify(data),
    });
  }

  delete(endpoint) {
    return this.request(endpoint, {
      method: 'DELETE',
    });
  }
}
