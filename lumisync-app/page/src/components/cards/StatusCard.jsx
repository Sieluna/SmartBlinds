import { splitProps, Show } from 'solid-js';

/**
 * Status card variants
 * @type {Object}
 */
const VARIANTS = {
  DEFAULT: 'default',
  SUCCESS: 'success',
  WARNING: 'warning',
  ERROR: 'error',
  INFO: 'info',
};

/**
 * Status card component for displaying status information
 * @param {Object} props - Component props
 */
export function StatusCard(props) {
  const [local, others] = splitProps(props, [
    'title',
    'value',
    'unit',
    'icon',
    'description',
    'variant',
    'trend',
    'trendValue',
    'className',
    'onClick',
    'loading',
  ]);

  const variant = () => local.variant ?? VARIANTS.DEFAULT;

  const getVariantClasses = () => {
    const baseClasses = 'p-4 rounded-lg shadow-sm border transition-all duration-200';

    const variantClasses = {
      [VARIANTS.DEFAULT]: 'bg-white border-gray-200 dark:bg-gray-800 dark:border-gray-700',
      [VARIANTS.SUCCESS]: 'bg-green-50 border-green-200 dark:bg-green-900/20 dark:border-green-800',
      [VARIANTS.WARNING]:
        'bg-yellow-50 border-yellow-200 dark:bg-yellow-900/20 dark:border-yellow-800',
      [VARIANTS.ERROR]: 'bg-red-50 border-red-200 dark:bg-red-900/20 dark:border-red-800',
      [VARIANTS.INFO]: 'bg-blue-50 border-blue-200 dark:bg-blue-900/20 dark:border-blue-800',
    };

    const interactiveClasses = local.onClick
      ? 'cursor-pointer hover:shadow-md active:scale-[0.98]'
      : '';

    return `${baseClasses} ${variantClasses[variant()]} ${interactiveClasses} ${local.className ?? ''}`;
  };

  const getTrendIcon = () => {
    if (!local.trend) return null;

    const trendClasses = {
      up: 'text-green-600 dark:text-green-400',
      down: 'text-red-600 dark:text-red-400',
      neutral: 'text-gray-600 dark:text-gray-400',
    };

    const trendIcons = {
      up: (
        <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
          <path
            d="M7 17L17 7M17 7H7M17 7V17"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
        </svg>
      ),
      down: (
        <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
          <path
            d="M7 7L17 17M17 17H7M17 17V7"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
        </svg>
      ),
      neutral: (
        <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
          <path
            d="M5 12H19"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
        </svg>
      ),
    };

    return (
      <span class={`inline-flex items-center ${trendClasses[local.trend]}`}>
        {trendIcons[local.trend]}
        {local.trendValue && <span class="ml-1">{local.trendValue}</span>}
      </span>
    );
  };

  return (
    <div class={getVariantClasses()} onClick={e => local?.onClick?.(e)} {...others}>
      <Show
        when={!local.loading}
        fallback={
          <div class="animate-pulse">
            <div class="h-4 bg-gray-200 dark:bg-gray-700 rounded w-3/4 mb-2.5" />
            <div class="h-8 bg-gray-200 dark:bg-gray-700 rounded w-1/2 mb-2.5" />
            <div class="h-4 bg-gray-200 dark:bg-gray-700 rounded w-full" />
          </div>
        }
      >
        <div class="flex items-start justify-between">
          <div>
            <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400">{local.title}</h3>
            <div class="flex items-baseline mt-1">
              <span class="text-2xl font-semibold">{local.value}</span>
              {local.unit && (
                <span class="ml-1 text-sm text-gray-600 dark:text-gray-400">{local.unit}</span>
              )}
            </div>
          </div>
          {local.icon && <div class="flex-shrink-0">{local.icon}</div>}
        </div>
        <Show when={local.description || local.trend}>
          <div class="mt-3 flex items-center justify-between">
            <p class="text-sm text-gray-600 dark:text-gray-400">{local.description}</p>
            {getTrendIcon()}
          </div>
        </Show>
      </Show>
    </div>
  );
}

// Export constants for external use
StatusCard.VARIANTS = VARIANTS;
