import { createSignal, onMount, For, Show, createMemo, batch } from 'solid-js';
import { produce } from 'solid-js/store';
import { invoke } from '@tauri-apps/api/core';
import { Button } from '../components/ui/Button.jsx';
import { List } from '../components/ui/List.jsx';
import { Form } from '../components/ui/Form.jsx';
import { useTranslation } from '../context/LanguageContext.jsx';

export function Wifi() {
  const { t } = useTranslation();
  const [wifiData, setWifiData] = createSignal(null);
  const [router, setRouter] = createSignal(null);
  const [devices, setDevices] = createSignal([]);
  const [isScanning, setIsScanning] = createSignal(false);
  const [showConfig, setShowConfig] = createSignal(false);

  const formStore = Form.createFormStore({
    routerPassword: '',
    deviceConfigs: {},
  });

  const getSignal = (entry) => {
    const rssi = entry?.access_points?.[0]?.links?.[0]?.rssi_dbm || -127;
    return Math.max(0, Math.min(100, ((rssi + 127) / 127) * 100));
  };

  const isConnected = (entry) => {
    const current = wifiData()?.current_connection;
    return current?.state === 'Connected' && entry.ssid === current?.ssid;
  };

  const needsPassword = () => {
    const entry = router() || {};
    return entry.security !== 'Open' && !entry.credential?.passphrase;
  };

  const hasSelections = () => router() || devices().length > 0;

  const scan = async () => {
    setIsScanning(true);
    try {
      setWifiData(await invoke('scan_wifis'));
    } catch {
      alert(t('wifi.errors.scanFailed'));
    } finally {
      setIsScanning(false);
    }
  };

  const toggleRouter = (entry) => {
    const current = router();
    const isSame = current && current.ssid === entry.ssid;

    batch(() => {
      setRouter(isSame ? null : entry);
      if (!isSame) formStore.setValue('routerPassword', '');
    });
  };

  const toggleDevice = (entry) => {
    const ssid = entry.ssid ?? '';

    setDevices((prev) => {
      const exists = prev.includes(ssid);
      if (exists) {
        formStore.setValue('deviceConfigs', produce(configs => {
          delete configs[ssid];
        }));
        return prev.filter((s) => s !== ssid);
      }

      const currentConfigs = formStore.values().deviceConfigs || {};
      formStore.setValue('deviceConfigs', {
        ...currentConfigs,
        [ssid]: {
          password: '',
          endpoint: 'http://192.168.71.1:80',
        }
      });
      return [...prev, ssid];
    });
  };

  const handleToggleConfig = (event) => {
    event.preventDefault();
    event.stopPropagation();
    setShowConfig(!showConfig());
  };

  const handleRemoveDevice = (entry, event) => {
    event.preventDefault();
    event.stopPropagation();
    toggleDevice(entry);
  };

  const configure = async (values) => {
    const routerEntry = router();
    if (!routerEntry) {
      alert(t('wifi.errors.noRouterSelected'));
      return;
    }

    try {
      const routerCreds = {
        ssid: routerEntry.ssid,
        security: routerEntry.security || 'Wpa2Personal',
        passphrase: needsPassword()
          ? values.routerPassword
          : routerEntry.credential?.passphrase || null,
        created_at: new Date().toISOString(),
        auto_connect: true,
        hidden: false,
      };

      const wifis = wifiData()?.wifis || {};

      for (const deviceSsid of devices()) {
        const deviceEntry = Object.values(wifis).find((entry) => entry.ssid === deviceSsid);

        if (!deviceEntry) continue;

        const config = values.deviceConfigs?.[deviceSsid] || {};

        await invoke('register_device', {
          device: {
            credentials: {
              ssid: deviceEntry.ssid,
              security: deviceEntry.security || 'Open',
              passphrase: config.password || null,
              created_at: new Date().toISOString(),
              auto_connect: false,
              hidden: false,
            },
            endpoint: config.endpoint || 'http://192.168.71.1:80',
          },
          routerCredentials: routerCreds,
        });
      }

      alert(t('wifi.success.devicesConfigured', { count: devices().length }));
      reset();
    } catch (error) {
      console.error('Configuration error:', error);
      alert(t('wifi.errors.configurationFailed', { error: error.toString() }));
    }
  };

  const reset = () => {
    batch(() => {
      setRouter(null);
      setDevices([]);
      setShowConfig(false);
      formStore.reset();
    });
  };

  const networks = createMemo(() => {
    const data = wifiData();
    if (!data?.wifis) return [];

    return Object.values(data.wifis)
      .filter((entry) => entry?.ssid)
      .map((entry) => {
        const ssid = entry.ssid ?? '';
        const signal = Math.round(getSignal(entry));
        const isRouter = router() && router().ssid === ssid;
        const isDevice = devices().includes(ssid);
        const connected = isConnected(entry);

        return {
          id: ssid,
          title: (
            <div class="flex items-center gap-2">
              <span>{ssid}</span>
              {connected && (
                <span class="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-300">
                  <div class="w-1.5 h-1.5 bg-green-500 rounded-full mr-1 animate-pulse" />
                  {t('wifi.connected')}
                </span>
              )}
            </div>
          ),
          description: `${signal}% • ${entry.security || 'Open'}`,
          action: (
            <div class="flex gap-2">
              <Button
                size="sm"
                variant={isRouter ? 'primary' : 'secondary'}
                onClick={() => toggleRouter(entry)}
              >
                {isRouter ? '✓' : t('wifi.router')}
              </Button>
              <Button
                size="sm"
                variant={isDevice ? 'primary' : 'ghost'}
                onClick={() => toggleDevice(entry)}
              >
                {isDevice ? '✓' : t('wifi.device')}
              </Button>
            </div>
          ),
        };
      });
  });

  const ConfigPanel = (props) => (
    <div class={props.class}>
      <h2 class="text-lg font-semibold mb-4">{t('wifi.configuration')}</h2>

      <Show when={router()}>
        <div class="mb-4 p-4 bg-blue-50 dark:bg-blue-900/20 rounded border border-blue-200 dark:border-blue-800">
          <div class="text-sm font-medium text-blue-900 dark:text-blue-100 mb-2">
            Router: {router().ssid ?? ''}
          </div>
          <Show when={needsPassword()}>
            <Form.Input
              name="routerPassword"
              type="password"
              placeholder={t('wifi.enterRouterPassword')}
              size="sm"
            />
          </Show>
        </div>
      </Show>

      <Show when={devices().length > 0}>
        <div class="mb-6">
          <div class="text-sm font-medium mb-3">
            {t('wifi.selectedDevicesCount', { count: devices().length })}
          </div>
          <div class="space-y-3 max-h-60 overflow-y-auto">
            <For each={devices()}>
              {(ssid) => {
                const entry = Object.values(wifiData()?.wifis || {}).find((e) => e.ssid === ssid);

                return (
                  <div class="p-3 border rounded">
                    <div class="flex justify-between items-center mb-2">
                      <span class="font-medium text-sm">{ssid}</span>
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={(event) => handleRemoveDevice(entry, event)}
                      >
                        ×
                      </Button>
                    </div>
                    <div class="space-y-2">
                      <Show when={entry?.security !== 'Open'}>
                        <Form.Input
                          name={`deviceConfigs.${ssid}.password`}
                          type="password"
                          placeholder={t('wifi.devicePassword')}
                          size="sm"
                        />
                      </Show>
                      <Form.Input
                        name={`deviceConfigs.${ssid}.endpoint`}
                        placeholder={t('wifi.deviceEndpoint')}
                        size="sm"
                      />
                    </div>
                  </div>
                );
              }}
            </For>
          </div>
        </div>
      </Show>

      <div class="space-y-2">
        <Form.SubmitButton fullWidth size={props.compact ? 'sm' : 'md'}>
          {t('wifi.configureDevices')}
        </Form.SubmitButton>
        <Button variant="secondary" fullWidth size={props.compact ? 'sm' : 'md'} onClick={reset}>
          {t('common.reset')}
        </Button>
      </div>
    </div>
  );

  onMount(scan);

  return (
    <div class="min-h-screen bg-gray-50 dark:bg-gray-900 pb-20 lg:pb-0">
      <div class="p-6">
        <h1 class="text-2xl font-bold mb-2">{t('wifi.title')}</h1>
        <p class="text-gray-600 dark:text-gray-400">{t('wifi.description')}</p>
      </div>

      <Form formStore={formStore} onSubmit={configure} class="max-w-6xl mx-auto px-6">
        <div class="lg:grid lg:grid-cols-3 lg:gap-6">
          {/* Networks List */}
          <div class="lg:col-span-2 mb-6 lg:mb-0">
            <div class="bg-white dark:bg-gray-800 rounded-lg border p-6">
              <div class="flex justify-between items-center mb-6">
                <h2 class="text-lg font-semibold">{t('wifi.availableNetworks')}</h2>
                <Button onClick={scan} loading={isScanning()} size="sm">
                  {isScanning() ? t('wifi.scanning') : t('wifi.refresh')}
                </Button>
              </div>

              <div class="max-h-96 overflow-y-auto">
                <Show
                  when={networks().length > 0}
                  fallback={
                    <div class="text-center py-8 text-gray-500">
                      {isScanning() ? t('wifi.scanningNetworks') : t('wifi.scanPrompt')}
                    </div>
                  }
                >
                  <List items={networks()} variant="divided" />
                </Show>
              </div>
            </div>
          </div>

          {/* Desktop Configuration Panel */}
          <Show when={hasSelections()}>
            <div class="hidden lg:block">
              <ConfigPanel class="bg-white dark:bg-gray-800 rounded-lg border p-6 sticky top-6" />
            </div>
          </Show>
        </div>

        {/* Mobile Bottom Panel */}
        <Show when={hasSelections()}>
          <div class="lg:hidden fixed bottom-0 left-0 right-0 z-50 bg-white dark:bg-gray-800 border-t shadow-lg">
            {/* Main Panel */}
            <div class="p-4 border-b border-gray-200 dark:border-gray-700">
              {/* Summary Bar */}
              <div class="flex justify-between items-center">
                <div class="flex gap-2 flex-wrap">
                  <Show when={router()}>
                    <span class="px-2 py-1 bg-blue-100 text-blue-800 text-xs rounded">
                      📡 {router().ssid ?? ''}
                    </span>
                  </Show>
                  <For each={devices().slice(0, 2)}>
                    {(ssid) => (
                      <span class="px-2 py-1 bg-green-100 text-green-800 text-xs rounded">
                        📱 {ssid}
                      </span>
                    )}
                  </For>
                  <Show when={devices().length > 2}>
                    <span class="px-2 py-1 bg-gray-100 text-gray-800 text-xs rounded">
                      +{devices().length - 2}
                    </span>
                  </Show>
                </div>
                <Button size="sm" variant="ghost" onClick={handleToggleConfig} type="button">
                  {showConfig() ? '▼' : '▲'}
                </Button>
              </div>
            </div>

            {/* Expanded Configuration */}
            <Show when={showConfig()}>
              <div class="max-h-96 overflow-y-auto p-4 bg-gray-50 dark:bg-gray-900">
                <ConfigPanel compact />
              </div>
            </Show>
          </div>
        </Show>
      </Form>
    </div>
  );
}
