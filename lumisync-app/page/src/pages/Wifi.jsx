import { createSignal, createEffect, onMount, For, Show } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';
import { Button } from '../components/ui/Button';
import { Input } from '../components/ui/Input';
import { List } from '../components/ui/List';

export function Wifi() {
  const [wifiData, setWifiData] = createSignal(null);
  const [selectedRouter, setSelectedRouter] = createSignal(null);
  const [routerPassword, setRouterPassword] = createSignal('');
  const [deviceCredentials, setDeviceCredentials] = createSignal({ ssid: '', password: '' });
  const [isScanning, setIsScanning] = createSignal(false);
  const [isRegistering, setIsRegistering] = createSignal(false);
  const [currentView, setCurrentView] = createSignal('scan'); // 'scan' | 'configure'

  // Scan WiFi networks
  const scanNetworks = async () => {
    setIsScanning(true);
    try {
      const result = await invoke('scan_wifis');
      setWifiData(result);
    } catch (error) {
      console.error('Failed to scan WiFi networks:', error);
    } finally {
      setIsScanning(false);
    }
  };

  // Register device with selected router credentials
  const registerDevice = async () => {
    if (!selectedRouter() || !routerPassword() || !deviceCredentials().ssid) {
      alert('Please fill in all required fields');
      return;
    }

    setIsRegistering(true);
    try {
      const device = {
        credentials: {
          ssid: deviceCredentials().ssid,
          security: 'Wpa2Personal',
          passphrase: deviceCredentials().password || null,
          created_at: new Date().toISOString(),
          auto_connect: false,
          hidden: false
        },
        endpoint: 'http://192.168.4.1:80' // Default ESP32 AP endpoint
      };

      const routerCreds = {
        ssid: selectedRouter().ssid,
        security: selectedRouter().security,
        passphrase: routerPassword(),
        created_at: new Date().toISOString(),
        auto_connect: true,
        hidden: false
      };

      await invoke('register_device', {
        device,
        routerCredentials: routerCreds
      });

      alert('Device registered successfully!');
      setSelectedRouter(null);
      setRouterPassword('');
      setDeviceCredentials({ ssid: '', password: '' });
      setCurrentView('scan');
    } catch (error) {
      alert(`Registration failed: ${error}`);
    } finally {
      setIsRegistering(false);
    }
  };

  // Get signal strength from access points with safe access
  const getSignalStrength = (networkEntry) => {
    // Safety check for the network entry
    if (!networkEntry) return 0;
    
    // Check if this is a flattened entry or has nested network
    const network = networkEntry.network || networkEntry;

    if (!network) return 0;
    
    if (!network.access_points || !Array.isArray(network.access_points) || network.access_points.length === 0)
      return 0;
    
    try {
      const maxRssi = Math.max(...network.access_points.map(ap => {
        if (!ap || !ap.links || !Array.isArray(ap.links)) {
          return -127; // Minimum RSSI
        }
        return Math.max(...ap.links.map(link => {
          if (!link || typeof link.rssi_dbm !== 'number') {
            return -127;
          }
          return link.rssi_dbm;
        }));
      }));
      
      // Convert RSSI (-127 to 0) to percentage (0 to 100)
      const percentage = Math.max(0, Math.min(100, ((maxRssi + 127) / 127) * 100));
      return percentage;
    } catch (error) {
      console.error('Error calculating signal strength:', error);
      return 0;
    }
  };

  // Get security display string
  const getSecurityString = (security) => {
    const securityMap = {
      'Open': 'Open',
      'Wep': 'WEP',
      'WpaPersonal': 'WPA',
      'Wpa2Personal': 'WPA2',
      'Wpa3Personal': 'WPA3',
      'WpaEnterprise': 'WPA Enterprise',
      'Wpa2Enterprise': 'WPA2 Enterprise',
      'Wpa3Enterprise': 'WPA3 Enterprise',
      'Unknown': 'Unknown'
    };
    return securityMap[security] || 'Unknown';
  };

  // Check if network is currently connected with safe access
  const isCurrentlyConnected = (networkEntry) => {
    const currentConnection = wifiData()?.current_connection;
    if (!currentConnection || currentConnection.state !== 'Connected') {
      return false;
    }
    
    // Handle both flattened and nested network structures
    const network = networkEntry.network || networkEntry;
    return currentConnection.ssid === network.ssid;
  };

  onMount(() => {
    scanNetworks();
  });

  // Signal strength icon component
  const SignalIcon = (props) => {
    const strength = props.strength;
    const bars = Math.ceil((strength / 100) * 4);
    
    return (
      <div class="flex items-end space-x-0.5 h-4 w-4">
        <For each={Array(4).fill(0)}>
          {(_, index) => (
            <div
              class={`w-0.5 bg-current transition-opacity ${
                index() < bars ? 'opacity-100' : 'opacity-30'
              }`}
              style={{ height: `${(index() + 1) * 25}%` }}
            />
          )}
        </For>
      </div>
    );
  };

  // Security icon component
  const SecurityIcon = (props) => {
    if (props.security !== 'Open') {
      return (
        <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
                d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 0 00-8 0v4h8z" />
        </svg>
      );
    }
    return null;
  };

  return (
    <div class="max-w-4xl mx-auto p-6 space-y-6">
      <div class="flex items-center justify-between">
        <h1 class="text-2xl font-bold text-gray-900 dark:text-gray-100">
          WiFi Device Setup
        </h1>
        <div class="flex space-x-2">
          <Button
            variant={currentView() === 'scan' ? 'primary' : 'secondary'}
            onClick={() => setCurrentView('scan')}
          >
            Scan Networks
          </Button>
          <Button
            variant={currentView() === 'configure' ? 'primary' : 'secondary'}
            onClick={() => setCurrentView('configure')}
            disabled={!selectedRouter()}
          >
            Configure Device
          </Button>
        </div>
      </div>

      <Show when={currentView() === 'scan'}>
        <div class="space-y-4">
          <div class="flex items-center justify-between">
            <h2 class="text-lg font-semibold text-gray-900 dark:text-gray-100">
              Available Networks
            </h2>
            <Button
              onClick={scanNetworks}
              loading={isScanning()}
              size="sm"
            >
              {isScanning() ? 'Scanning...' : 'Refresh'}
            </Button>
          </div>

          <Show when={wifiData()?.current_connection}>
            <div class="bg-blue-50 dark:bg-blue-900/20 p-4 rounded-lg border border-blue-200 dark:border-blue-800">
              <h3 class="font-medium text-blue-900 dark:text-blue-100 mb-2">
                Current Connection
              </h3>
              <p class="text-blue-700 dark:text-blue-300 text-sm">
                Connected to: {wifiData().current_connection.ssid || 'Unknown'}
                <Show when={wifiData().current_connection.state !== 'Connected'}>
                  <span class="ml-2 text-orange-600">
                    ({wifiData().current_connection.state})
                  </span>
                </Show>
              </p>
            </div>
          </Show>

          <Show when={wifiData()?.wifis}>
            <List
              items={Object.values(wifiData().wifis || {}).filter(Boolean).map(entry => {
                console.log('Processing entry:', JSON.stringify(entry, null, 2));
                
                // Handle both flattened and nested network structures
                const network = entry.network || entry;
                if (!network || !network.ssid) {
                  console.warn('Invalid network entry:', entry);
                  return null;
                }
                
                const signalStrength = getSignalStrength(entry);
                const isConnected = isCurrentlyConnected(entry);
                
                return {
                  ...network,
                  title: network.ssid,
                  description: `Signal: ${Math.round(signalStrength)}% | Security: ${getSecurityString(network.security)}`,
                  icon: <SignalIcon strength={signalStrength} />,
                  action: (
                    <div class="flex items-center space-x-2">
                      <SecurityIcon security={network.security} />
                      <Show when={isConnected}>
                        <span class="text-green-600 text-sm font-medium">Connected</span>
                      </Show>
                      <Show when={!isConnected}>
                        <Button
                          size="sm"
                          variant={selectedRouter()?.ssid === network.ssid ? 'primary' : 'secondary'}
                          onClick={() => setSelectedRouter(network)}
                        >
                          {selectedRouter()?.ssid === network.ssid ? 'Selected' : 'Select'}
                        </Button>
                      </Show>
                    </div>
                  )
                };
              }).filter(Boolean)}
              variant="bordered"
              hoverable
            />
          </Show>

          <Show when={!wifiData() && !isScanning()}>
            <div class="text-center py-8 text-gray-500 dark:text-gray-400">
              Click "Refresh" to scan for WiFi networks
            </div>
          </Show>
        </div>
      </Show>

      <Show when={currentView() === 'configure'}>
        <div class="space-y-6">
          <h2 class="text-lg font-semibold text-gray-900 dark:text-gray-100">
            Configure Device
          </h2>

          <Show when={selectedRouter()}>
            <div class="bg-white dark:bg-gray-800 p-6 rounded-lg border border-gray-200 dark:border-gray-700 space-y-4">
              <h3 class="text-lg font-semibold text-gray-900 dark:text-gray-100">
                Router Network: "{selectedRouter().ssid}"
              </h3>
              
              <div class="space-y-3">
                <div>
                  <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                    WiFi Password
                  </label>
                  <Input
                    type="password"
                    placeholder="Enter router WiFi password"
                    value={routerPassword()}
                    onInput={(e) => setRouterPassword(e.target.value)}
                    fullWidth
                  />
                </div>
              </div>
            </div>
          </Show>

          <div class="bg-white dark:bg-gray-800 p-6 rounded-lg border border-gray-200 dark:border-gray-700 space-y-4">
            <h3 class="text-lg font-semibold text-gray-900 dark:text-gray-100">
              Device Access Point
            </h3>
            <p class="text-sm text-gray-600 dark:text-gray-400">
              Connect to your device's WiFi hotspot to configure it
            </p>
            
            <div class="space-y-3">
              <div>
                <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                  Device SSID
                </label>
                <Input
                  placeholder="e.g., SmartBlinds, ESP32-Setup"
                  value={deviceCredentials().ssid}
                  onInput={(e) => setDeviceCredentials(prev => ({ ...prev, ssid: e.target.value }))}
                  fullWidth
                />
              </div>
              
              <div>
                <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                  Device Password (Optional)
                </label>
                <Input
                  type="password"
                  placeholder="Device AP password (if any)"
                  value={deviceCredentials().password}
                  onInput={(e) => setDeviceCredentials(prev => ({ ...prev, password: e.target.value }))}
                  fullWidth
                />
              </div>
            </div>
          </div>

          <div class="flex space-x-3">
            <Button
              onClick={registerDevice}
              loading={isRegistering()}
              disabled={!selectedRouter() || !routerPassword() || !deviceCredentials().ssid}
              fullWidth
            >
              {isRegistering() ? 'Configuring Device...' : 'Configure Device'}
            </Button>
            <Button
              variant="secondary"
              onClick={() => setCurrentView('scan')}
              disabled={isRegistering()}
            >
              Back
            </Button>
          </div>

          <div class="bg-yellow-50 dark:bg-yellow-900/20 p-4 rounded-lg border border-yellow-200 dark:border-yellow-800">
            <h4 class="font-medium text-yellow-900 dark:text-yellow-100 mb-2">
              Setup Instructions
            </h4>
            <ol class="text-yellow-700 dark:text-yellow-300 text-sm space-y-1 list-decimal list-inside">
              <li>Put your device in setup mode</li>
              <li>Connect to the device's WiFi hotspot</li>
              <li>Click "Configure Device" to send router credentials</li>
              <li>Wait for the device to connect to your router</li>
            </ol>
          </div>
        </div>
      </Show>
    </div>
  );
}
