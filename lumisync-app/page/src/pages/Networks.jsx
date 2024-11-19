import { createSignal, onMount, For, Show } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';
import { Button } from '../components/ui/Button.jsx';
import { List } from '../components/ui/List.jsx';
import { useTranslation } from '../context/LanguageContext.jsx';

export function Networks() {
  const { t } = useTranslation();
  const [networkData, setNetworkData] = createSignal(null);
  const [isScanning, setIsScanning] = createSignal(false);

  const getSignalQuality = (rssi) => {
    // Convert RSSI to percentage (rough approximation)
    return Math.max(0, Math.min(100, ((rssi + 127) / 127) * 100));
  };

  const scan = async () => {
    setIsScanning(true);
    try {
      const result = await invoke('scan_networks');
      setNetworkData(result);
    } catch (error) {
      console.error('Network scan failed:', error);
      alert(`Scan failed: ${error}`);
    } finally {
      setIsScanning(false);
    }
  };

  const networks = () => {
    const data = networkData();
    if (!data?.networks) return [];

    return data.networks
      .sort((a, b) => b.signal_strength - a.signal_strength)
      .map((network) => {
        const signalQuality = Math.round(getSignalQuality(network.signal_strength));
        
        return {
          id: network.ssid,
          title: (
            <div class="flex items-center gap-2">
              <span>{network.ssid}</span>
              {network.is_connected && (
                <span class="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-300">
                  <div class="w-1.5 h-1.5 bg-green-500 rounded-full mr-1 animate-pulse" />
                  Connected
                </span>
              )}
            </div>
          ),
          description: (
            <div class="space-y-1">
              <div class="flex items-center gap-4 text-sm text-gray-600 dark:text-gray-400">
                <span>Signal: {signalQuality}%</span>
                <span>Security: {network.security}</span>
                {network.frequency && (
                  <span>Frequency: {network.frequency} MHz</span>
                )}
              </div>
              <div class="text-xs text-gray-500">
                RSSI: {network.signal_strength} dBm
              </div>
            </div>
          ),
        };
      });
  };

  onMount(scan);

  return (
    <div class="min-h-screen bg-gray-50 dark:bg-gray-900 pb-20">
      <div class="p-6">
        <h1 class="text-2xl font-bold mb-2">Network Scanner</h1>
        <p class="text-gray-600 dark:text-gray-400 mb-6">
          Scan and view available WiFi networks in your area
        </p>

        <div class="bg-white dark:bg-gray-800 rounded-lg border p-6">
          <div class="flex justify-between items-center mb-6">
            <h2 class="text-lg font-semibold">Available Networks</h2>
            <div class="flex items-center gap-4">
              {networkData() && (
                <span class="text-sm text-gray-500">
                  {networks().length} networks found
                </span>
              )}
              <Button onClick={scan} loading={isScanning()} size="sm">
                {isScanning() ? 'Scanning...' : 'Scan Networks'}
              </Button>
            </div>
          </div>

          <div class="max-h-96 overflow-y-auto">
            <Show
              when={networks().length > 0}
              fallback={
                <div class="text-center py-8 text-gray-500">
                  {isScanning() ? 'Scanning for networks...' : 'No networks found. Click "Scan Networks" to start.'}
                </div>
              }
            >
              <List items={networks()} variant="divided" />
            </Show>
          </div>

          {networkData() && (
            <div class="mt-4 p-3 bg-gray-50 dark:bg-gray-900 rounded text-sm text-gray-600 dark:text-gray-400">
              <div class="flex justify-between">
                <span>Scan completed at:</span>
                <span>{new Date(networkData().scan_timestamp).toLocaleString()}</span>
              </div>
              {networkData().current_connection && (
                <div class="flex justify-between mt-1">
                  <span>Currently connected to:</span>
                  <span class="font-medium">{networkData().current_connection}</span>
                </div>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
} 