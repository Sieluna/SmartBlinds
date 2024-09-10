import { WS_BASE_URL } from './constants.js';

export class WebSocketManager {
  constructor(dispatch) {
    this.dispatch = dispatch;
    this.connections = new Map();
  }

  connect(endpoint, onMessage) {
    this.disconnect(endpoint);

    const ws = new WebSocket(`${WS_BASE_URL}${endpoint}?token=${this.getToken()}`);

    ws.onmessage = event => {
      try {
        const data = JSON.parse(event.data);
        onMessage(data);
        this.dispatch({
          type: 'ENTITIES/UPDATE',
          payload: {
            sensors: { [data.sensorId]: data },
          },
        });
      } catch (error) {
        console.error('WebSocket message parsing failed:', error);
      }
    };

    ws.onclose = () => this.reconnect(endpoint, onMessage);
    this.connections.set(endpoint, ws);
  }

  reconnect(endpoint, onMessage) {
    setTimeout(() => this.connect(endpoint, onMessage), 5000);
  }

  disconnect(endpoint) {
    if (this.connections.has(endpoint)) {
      this.connections.get(endpoint).close();
      this.connections.delete(endpoint);
    }
  }
}
