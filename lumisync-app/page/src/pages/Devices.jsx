import { createSignal, For, Show, onMount } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';
import { Button } from '../components/ui/Button.jsx';
import { Form } from '../components/ui/Form.jsx';
import { List } from '../components/ui/List.jsx';
import { useTranslation } from '../context/LanguageContext.jsx';

export function Devices() {
  const { t } = useTranslation();
  const [devices, setDevices] = createSignal([]);
  const [savedProfiles, setSavedProfiles] = createSignal([]);
  const [isScanning, setIsScanning] = createSignal(false);
  const [isConfiguring, setIsConfiguring] = createSignal(false);
  const [selectedDevice, setSelectedDevice] = createSignal(null);
  const [showConfigForm, setShowConfigForm] = createSignal(false);

  const formStore = Form.createFormStore({
    routerSSID: '',
    routerPassword: '',
    useSmartConfig: true,
  });

  const loadSavedProfiles = async () => {
    try {
      const result = await invoke('manage_device', {
        action: 'ListProfiles'
      });
      setSavedProfiles(result.profiles || []);
      console.log(`Loaded ${result.count} cached profiles`);
    } catch (error) {
      console.error('Failed to load saved profiles:', error);
    }
  };

  const configureDevice = async (values) => {
    const device = selectedDevice();
    if (!device) return;

    setIsConfiguring(true);
    try {
      const result = await invoke('manage_device', {
        action: {
          Configure: {
            device_ssid: device.ssid,
            router_ssid: values.routerSSID,
            router_password: values.routerPassword || null
          }
        }
      });
      
      let message = 'Device configured successfully!';
      if (result.auto_password && result.password_required) {
        message += '\n✓ Used cached password automatically';
      } else if (!result.password_required) {
        message += '\n✓ Network is open (no password required)';
      }
      
      alert(message);
      setShowConfigForm(false);
      setSelectedDevice(null);
      formStore.reset();
      
      // Refresh profiles cache after successful configuration
      await loadSavedProfiles();
    } catch (error) {
      console.error('Device configuration failed:', error);
      let errorMessage = `Configuration failed: ${error}`;
      
      // Provide helpful hints based on error type
      if (error.toString().includes('requires a password')) {
        errorMessage += '\n\nTip: This network needs a password. Either:\n• Enter the password manually\n• Save the network credentials first and try again';
      }
      
      alert(errorMessage);
    } finally {
      setIsConfiguring(false);
    }
  };

  const selectSavedNetwork = (profile) => {
    formStore.setValue('routerSSID', profile.ssid);
    if (profile.has_password) {
      // Clear password field to show we'll use cached password
      formStore.setValue('routerPassword', '');
      formStore.setValue('useSmartConfig', true);
    } else {
      // Open network
      formStore.setValue('routerPassword', '');
      formStore.setValue('useSmartConfig', true);
    }
  };

  const discoverDevices = async () => {
    setIsScanning(true);
    try {
      const result = await invoke('manage_device', {
        action: 'Discover'
      });
      
      setDevices(result.devices || []);
    } catch (error) {
      console.error('Device discovery failed:', error);
      alert(`Discovery failed: ${error}`);
    } finally {
      setIsScanning(false);
    }
  };

  const connectToNetwork = async (ssid, password = '') => {
    try {
      await invoke('manage_device', {
        action: {
          Connect: {
            ssid,
            password
          }
        }
      });
      
      alert(`Connected to ${ssid}`);
    } catch (error) {
      console.error('Connection failed:', error);
      alert(`Connection failed: ${error}`);
    }
  };

  const disconnect = async () => {
    try {
      await invoke('manage_device', {
        action: 'Disconnect'
      });
      
      alert('Disconnected from network');
    } catch (error) {
      console.error('Disconnect failed:', error);
      alert(`Disconnect failed: ${error}`);
    }
  };

  const getSignalQuality = (rssi) => {
    return Math.max(0, Math.min(100, ((rssi + 127) / 127) * 100));
  };

  const deviceItems = () => {
    return devices().map((device) => {
      const signalQuality = Math.round(getSignalQuality(device.signal_strength));
      
      return {
        id: device.ssid,
        title: (
          <div class="flex items-center gap-2">
            <span class="font-medium">{device.ssid}</span>
            <span class="px-2 py-1 bg-blue-100 text-blue-800 dark:bg-blue-900/30 dark:text-blue-300 text-xs rounded">
              {device.device_type}
            </span>
          </div>
        ),
        description: (
          <div class="space-y-2">
            <div class="flex items-center gap-4 text-sm text-gray-600 dark:text-gray-400">
              <span>Signal: {signalQuality}% ({device.signal_strength} dBm)</span>
              <span>Security: {device.security}</span>
              <span>Endpoint: {device.endpoint}</span>
            </div>
          </div>
        ),
        action: (
          <div class="flex gap-2">
            <Button
              size="sm"
              variant="primary"
              onClick={() => {
                setSelectedDevice(device);
                setShowConfigForm(true);
              }}
            >
              Configure
            </Button>
            <Button
              size="sm"
              variant="secondary"
              onClick={() => connectToNetwork(device.ssid)}
            >
              Connect
            </Button>
          </div>
        ),
      };
    });
  };

  onMount(() => {
    loadSavedProfiles();
  });

  const ConfigurationModal = () => (
    <Show when={showConfigForm()}>
      <div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
        <div class="bg-white dark:bg-gray-800 rounded-lg p-6 w-full max-w-md mx-4 max-h-[90vh] overflow-y-auto">
          <h3 class="text-lg font-semibold mb-4">
            Configure Device: {selectedDevice()?.ssid}
          </h3>
          
          <Show when={savedProfiles().length > 0}>
            <div class="mb-4 p-3 bg-gray-50 dark:bg-gray-900 rounded">
              <h4 class="text-sm font-medium mb-2">
                Quick Select (Cached Networks)
                <span class="ml-2 text-xs text-gray-500">({savedProfiles().length} saved)</span>
              </h4>
              <div class="space-y-1 max-h-32 overflow-y-auto">
                <For each={savedProfiles()}>
                  {(profile) => (
                    <button
                      class="w-full text-left px-2 py-1 text-xs bg-white dark:bg-gray-800 border rounded hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors"
                      onClick={() => selectSavedNetwork(profile)}
                    >
                      <div class="flex justify-between items-center">
                        <span class="font-medium">{profile.ssid}</span>
                        <div class="flex items-center gap-1">
                          <span class="text-gray-500">
                            {profile.has_password ? '🔒' : '🔓'}
                          </span>
                          {profile.auto_connect && (
                            <span class="text-green-500 text-xs">●</span>
                          )}
                        </div>
                      </div>
                      <div class="text-xs text-gray-500 mt-1">
                        {profile.security} • Created: {new Date(profile.created_at).toLocaleDateString()}
                      </div>
                    </button>
                  )}
                </For>
              </div>
            </div>
          </Show>
          
          <Form formStore={formStore} onSubmit={configureDevice}>
            <div class="space-y-4">
              <Form.Input
                name="routerSSID"
                label="Router WiFi Name (SSID)"
                placeholder="Enter your home WiFi network name"
                required
              />
              
              <div class="bg-blue-50 dark:bg-blue-900/20 p-3 rounded text-sm">
                <div class="flex items-center gap-2 mb-2">
                  <input
                    type="checkbox"
                    checked={formStore.values.useSmartConfig}
                    onChange={(e) => formStore.setValue('useSmartConfig', e.target.checked)}
                    class="rounded"
                  />
                  <label class="font-medium text-blue-900 dark:text-blue-100">
                    Smart Configuration
                  </label>
                </div>
                <p class="text-blue-800 dark:text-blue-200 text-xs">
                  Automatically uses cached passwords and detects network security requirements
                </p>
              </div>
              
              <Form.Input
                name="routerPassword"
                type="password"
                label="Router WiFi Password"
                placeholder={
                  formStore.values.useSmartConfig 
                    ? "Leave empty to use cached password (if available)" 
                    : "Enter your home WiFi password"
                }
              />
            </div>
            
            <div class="flex gap-3 mt-6">
              <Form.SubmitButton 
                fullWidth 
                loading={isConfiguring()}
              >
                {formStore.values.useSmartConfig ? '🚀 Smart Configure' : 'Configure Device'}
              </Form.SubmitButton>
              <Button
                variant="secondary"
                fullWidth
                onClick={() => {
                  setShowConfigForm(false);
                  setSelectedDevice(null);
                  formStore.reset();
                }}
                disabled={isConfiguring()}
              >
                Cancel
              </Button>
            </div>
          </Form>
        </div>
      </div>
    </Show>
  );

  return (
    <div class="min-h-screen bg-gray-50 dark:bg-gray-900 pb-20">
      <div class="p-6">
        <h1 class="text-2xl font-bold mb-2">Device Manager</h1>
        <p class="text-gray-600 dark:text-gray-400 mb-6">
          Discover and configure SmartBlinds and other IoT devices with intelligent password management
        </p>

        <div class="bg-white dark:bg-gray-800 rounded-lg border p-6 mb-6">
          <div class="flex justify-between items-center mb-6">
            <h2 class="text-lg font-semibold">Device Discovery</h2>
            <div class="flex items-center gap-4">
              {devices().length > 0 && (
                <span class="text-sm text-gray-500">
                  {devices().length} device{devices().length !== 1 ? 's' : ''} found
                </span>
              )}
              <Button onClick={loadSavedProfiles} variant="ghost" size="sm">
                🔄 Refresh Cache
              </Button>
              <Button onClick={disconnect} variant="ghost" size="sm">
                Disconnect
              </Button>
              <Button onClick={discoverDevices} loading={isScanning()} size="sm">
                {isScanning() ? 'Scanning...' : 'Discover Devices'}
              </Button>
            </div>
          </div>

          <div class="max-h-96 overflow-y-auto">
            <Show
              when={deviceItems().length > 0}
              fallback={
                <div class="text-center py-8 text-gray-500">
                  {isScanning() 
                    ? 'Scanning for SmartBlinds devices...' 
                    : 'No devices found. Click "Discover Devices" to scan for nearby SmartBlinds controllers.'
                  }
                </div>
              }
            >
              <List items={deviceItems()} variant="divided" />
            </Show>
          </div>
        </div>

        {/* Enhanced Configuration Instructions */}
        <div class="bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg p-6">
          <h3 class="font-semibold text-blue-900 dark:text-blue-100 mb-2">
            🚀 Smart Configuration Features
          </h3>
          <div class="grid md:grid-cols-2 gap-4 text-sm text-blue-800 dark:text-blue-200">
            <div>
              <h4 class="font-medium mb-2">Automatic Features:</h4>
              <ul class="space-y-1">
                <li>• Cached password lookup</li>
                <li>• Network security detection</li>
                <li>• Connection restoration</li>
                <li>• Smart error handling</li>
              </ul>
            </div>
            <div>
              <h4 class="font-medium mb-2">Quick Setup:</h4>
              <ol class="space-y-1">
                <li>1. Click "Discover Devices"</li>
                <li>2. Select a device to configure</li>
                <li>3. Choose from cached networks or enter new</li>
                <li>4. Let smart config handle the rest!</li>
              </ol>
            </div>
          </div>
        </div>
      </div>

      <ConfigurationModal />
    </div>
  );
} 