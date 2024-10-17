import { splitProps } from 'solid-js';

import { useTranslation } from '../../context/LanguageContext.jsx';

import { StatusCard } from './StatusCard.jsx';

/**
 * Device status card for displaying device information
 * @param {Object} props - Component props
 */
export function DeviceStatusCard(props) {
  const { t } = useTranslation();

  const [local, others] = splitProps(props, ['device', 'onClick', 'loading', 'className']);

  /**
   * Format device value based on type
   * @param {string} type - Value type
   * @param {number|string} value - Device value
   * @returns {string} Formatted value
   */
  const formatValue = (type, value) => {
    if (value === undefined || value === null) return '--';

    switch (type) {
      case 'temperature':
        return Number(value).toFixed(1);
      case 'humidity':
      case 'position':
      case 'battery':
        return Math.round(Number(value));
      case 'illuminance':
        return value > 1000 ? `${(value / 1000).toFixed(1)}k` : value;
      default:
        return value.toString();
    }
  };

  /**
   * Get trend direction based on previous value
   * @param {string} type - Value type
   * @param {number|string} current - Current value
   * @param {number|string} previous - Previous value
   * @returns {string|null} Trend direction
   */
  const getTrend = (type, current, previous) => {
    if (previous === undefined || previous === null) return null;

    const diff = Number(current) - Number(previous);
    if (Math.abs(diff) < 0.01) return 'neutral';

    // For temperature and humidity, we want to show whether it's increasing or decreasing
    return diff > 0 ? 'up' : 'down';
  };

  /**
   * Get formatted trend value
   * @param {string} type - Value type
   * @param {number|string} current - Current value
   * @param {number|string} previous - Previous value
   * @returns {string|null} Formatted trend value
   */
  const getTrendValue = (type, current, previous) => {
    if (previous === undefined || previous === null) return null;

    const diff = Number(current) - Number(previous);
    if (Math.abs(diff) < 0.01) return null;

    const absValue = Math.abs(diff);

    switch (type) {
      case 'temperature':
        return `${absValue.toFixed(1)}°`;
      case 'humidity':
      case 'position':
      case 'battery':
        return `${Math.round(absValue)}%`;
      case 'illuminance':
        return absValue > 1000 ? `${(absValue / 1000).toFixed(1)}k` : `${Math.round(absValue)}`;
      default:
        return null;
    }
  };

  /**
   * Get unit for the given type
   * @param {string} type - Value type
   * @returns {string} Unit
   */
  const getUnit = type => {
    switch (type) {
      case 'temperature':
        return '°C';
      case 'humidity':
      case 'position':
      case 'battery':
        return '%';
      case 'illuminance':
        return 'lux';
      default:
        return '';
    }
  };

  /**
   * Get icon for the given device type
   * @returns {JSX.Element} Icon element
   */
  const getDeviceIcon = () => {
    switch (local.device?.device_type) {
      case 'window':
        return (
          <svg
            xmlns="http://www.w3.org/2000/svg"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            class="w-6 h-6 text-blue-500"
          >
            <rect x="2" y="4" width="20" height="16" rx="2" />
            <line x1="12" y1="4" x2="12" y2="20" />
          </svg>
        );
      case 'sensor':
        return (
          <svg
            xmlns="http://www.w3.org/2000/svg"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            class="w-6 h-6 text-green-500"
          >
            <path d="M12 2L4 6l8 4 8-4-8-4z" />
            <path d="M4 12l8 4 8-4" />
            <path d="M4 18l8 4 8-4" />
          </svg>
        );
      default:
        return (
          <svg
            xmlns="http://www.w3.org/2000/svg"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            class="w-6 h-6 text-gray-500"
          >
            <circle cx="12" cy="12" r="10" />
            <path d="M12 16v-4" />
            <path d="M12 8h.01" />
          </svg>
        );
    }
  };

  /**
   * Get status variant based on device status
   * @returns {string} Status variant
   */
  const getStatusVariant = () => {
    if (!local.device) return StatusCard.VARIANTS.DEFAULT;

    const batteryLevel = Number(local.device.battery);
    if (batteryLevel < 10) return StatusCard.VARIANTS.ERROR;
    if (batteryLevel < 20) return StatusCard.VARIANTS.WARNING;

    if (local.device.status === 'offline') return StatusCard.VARIANTS.ERROR;
    if (local.device.status === 'warning') return StatusCard.VARIANTS.WARNING;

    return StatusCard.VARIANTS.DEFAULT;
  };

  /**
   * Get primary display value for the device
   * @returns {Object} Value and unit
   */
  const getPrimaryValue = () => {
    if (!local.device) return { value: '--', unit: '', type: '' };

    switch (local.device.device_type) {
      case 'window':
        return {
          value: formatValue('position', local.device.position),
          unit: getUnit('position'),
          type: 'position',
        };
      case 'sensor':
        return {
          value: formatValue('temperature', local.device.temperature),
          unit: getUnit('temperature'),
          type: 'temperature',
        };
      default:
        return { value: '--', unit: '', type: '' };
    }
  };

  /**
   * Get status description for the device
   * @returns {string} Status description
   */
  const getStatusDescription = () => {
    if (!local.device) return '';

    const timestamp = local.device.updated_at
      ? new Date(local.device.updated_at).toLocaleTimeString()
      : '';

    return `${t('device.battery')}: ${local.device.battery}% | ${t('device.lastUpdated')}: ${timestamp}`;
  };

  return (
    <StatusCard
      title={local.device?.name ?? 'Unknown Device'}
      value={getPrimaryValue().value}
      unit={getPrimaryValue().unit}
      variant={getStatusVariant()}
      icon={getDeviceIcon()}
      description={getStatusDescription()}
      trend={
        local.device?.previous &&
        getTrend(getPrimaryValue().type, local.device[getPrimaryValue().type], local.device.previous[getPrimaryValue().type])
      }
      trendValue={
        local.device?.previous &&
        getTrendValue(getPrimaryValue().type, local.device[getPrimaryValue().type], local.device.previous[getPrimaryValue().type])
      }
      loading={local.loading}
      onClick={local.onClick}
      class={local.className}
      {...others}
    />
  );
}
