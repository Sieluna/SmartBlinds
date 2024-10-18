import {
  splitProps,
  mergeProps,
  For,
  createMemo,
  createUniqueId,
  Show,
  createSignal,
} from 'solid-js';

/**
 * Available select size options
 * @constant {Object}
 */
const SIZES = {
  XS: 'xs',
  SM: 'sm',
  MD: 'md',
  LG: 'lg',
  XL: 'xl',
};

/**
 * Available select types
 * @constant {Object}
 */
const TYPES = {
  SELECT: 'select',
  RADIO: 'radio',
  CHECKBOX: 'checkbox',
  MULTI_SELECT: 'multi-select',
};

/**
 * Available select variants (for dropdown types)
 * @constant {Object}
 */
const VARIANTS = {
  DEFAULT: 'default',
  FILLED: 'filled',
  UNDERLINED: 'underlined',
};

/**
 * Available layout orientations (for radio/checkbox groups)
 * @constant {Object}
 */
const ORIENTATIONS = {
  HORIZONTAL: 'horizontal',
  VERTICAL: 'vertical',
};

/**
 * Unified Select component that handles different selection types
 * @param {Object} props - Component properties
 */
export function Select(props) {
  const merged = mergeProps(
    {
      type: TYPES.SELECT,
      size: SIZES.MD,
      variant: VARIANTS.DEFAULT,
      orientation: ORIENTATIONS.VERTICAL,
      options: [],
      disabled: false,
      error: false,
    },
    props
  );

  const [local, others] = splitProps(merged, [
    'class',
    'type',
    'size',
    'variant',
    'orientation',
    'options',
    'placeholder',
    'label',
    'description',
    'icon',
    'error',
    'disabled',
    'ref',
    'id',
    'name',
    'value',
    'onChange',
  ]);

  const selectId = createUniqueId();
  const id = () => local.id || selectId;
  const name = () => local.name || `select-${id()}`;

  // Common styles for different input types
  const getInputClasses = createMemo(() => {
    const sizes = {
      [SIZES.XS]: 'h-3 w-3',
      [SIZES.SM]: 'h-4 w-4',
      [SIZES.MD]: 'h-4 w-4',
      [SIZES.LG]: 'h-5 w-5',
      [SIZES.XL]: 'h-6 w-6',
    };

    const base = ['transition-colors', 'focus:ring-2', 'focus:ring-offset-2', sizes[local.size]];

    const states = [];
    if (local.error) {
      states.push(
        'border-red-300',
        'dark:border-red-600',
        'focus:ring-red-500',
        'dark:focus:ring-red-400'
      );
    } else {
      states.push(
        'border-gray-300',
        'dark:border-gray-600',
        'text-blue-600',
        'focus:ring-blue-500',
        'dark:focus:ring-blue-400'
      );
    }

    if (local.disabled) {
      states.push('opacity-50', 'cursor-not-allowed');
    }

    return [...base, ...states].filter(Boolean).join(' ');
  });

  // Dropdown select styles
  const getSelectClasses = createMemo(() => {
    const base = [
      'block',
      'w-full',
      'bg-white',
      'dark:bg-gray-900',
      'text-gray-900',
      'dark:text-gray-100',
      'focus:outline-none',
      'appearance-none',
      'cursor-pointer',
      'transition-colors',
      'disabled:opacity-50',
      'disabled:cursor-not-allowed',
    ];

    const sizes = {
      [SIZES.XS]: ['px-2', 'py-1', 'pr-6', 'text-xs'],
      [SIZES.SM]: ['px-2.5', 'py-1.5', 'pr-7', 'text-sm'],
      [SIZES.MD]: ['px-3', 'py-2', 'pr-10', 'text-sm'],
      [SIZES.LG]: ['px-4', 'py-2.5', 'pr-12', 'text-base'],
      [SIZES.XL]: ['px-5', 'py-3', 'pr-14', 'text-lg'],
    };

    const variants = {
      [VARIANTS.DEFAULT]: [
        'border',
        'border-gray-300',
        'dark:border-gray-600',
        'rounded-md',
        'shadow-sm',
      ],
      [VARIANTS.FILLED]: ['border-0', 'rounded-md', 'bg-gray-100', 'dark:bg-gray-800'],
      [VARIANTS.UNDERLINED]: [
        'border-0',
        'border-b-2',
        'border-gray-300',
        'dark:border-gray-600',
        'rounded-none',
        'bg-transparent',
        'px-0',
      ],
    };

    const states = local.error
      ? ['border-red-300', 'dark:border-red-600', 'focus:ring-red-500', 'focus:border-red-500']
      : [
          'focus:ring-blue-500',
          'focus:border-blue-500',
          'dark:focus:ring-blue-400',
          'dark:focus:border-blue-400',
        ];

    return [...base, ...sizes[local.size], ...variants[local.variant], ...states, local.class]
      .filter(Boolean)
      .join(' ');
  });

  // Label styles
  const getLabelClasses = createMemo(() => {
    const sizes = {
      [SIZES.XS]: 'text-xs',
      [SIZES.SM]: 'text-sm',
      [SIZES.MD]: 'text-sm',
      [SIZES.LG]: 'text-base',
      [SIZES.XL]: 'text-lg',
    };

    const base = ['select-none', sizes[local.size]];

    if (local.disabled) {
      base.push('text-gray-400', 'cursor-not-allowed');
    } else {
      base.push('text-gray-700', 'dark:text-gray-300', 'cursor-pointer');
    }

    return base.join(' ');
  });

  // Container styles for radio/checkbox groups
  const getGroupClasses = createMemo(() => {
    const base = ['flex'];

    const orientations = {
      [ORIENTATIONS.HORIZONTAL]: ['flex-row', 'flex-wrap', 'gap-4'],
      [ORIENTATIONS.VERTICAL]: ['flex-col', 'space-y-2'],
    };

    return [...base, ...orientations[local.orientation]].filter(Boolean).join(' ');
  });

  // Icon for dropdown
  const renderDropdownIcon = () => {
    if (local.icon) return local.icon;

    const iconSizes = {
      [SIZES.XS]: 'h-3 w-3',
      [SIZES.SM]: 'h-4 w-4',
      [SIZES.MD]: 'h-5 w-5',
      [SIZES.LG]: 'h-6 w-6',
      [SIZES.XL]: 'h-7 w-7',
    };

    return (
      <svg
        class={`${iconSizes[local.size]} text-gray-400`}
        xmlns="http://www.w3.org/2000/svg"
        viewBox="0 0 20 20"
        fill="currentColor"
      >
        <path
          fill-rule="evenodd"
          d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z"
          clip-rule="evenodd"
        />
      </svg>
    );
  };

  // Handle change events
  const handleChange = (event) => {
    const { value, checked } = event.target;

    if (local.type === TYPES.CHECKBOX && local.options.length === 0) {
      // Single checkbox
      local.onChange?.(checked);
    } else if (
      local.type === TYPES.MULTI_SELECT ||
      (local.type === TYPES.CHECKBOX && local.options.length > 0)
    ) {
      // Multi-select handling (both explicit multi-select and checkbox groups)
      const currentValues = Array.isArray(local.value) ? local.value : [];
      const newValues = checked
        ? [...currentValues, value]
        : currentValues.filter((v) => v !== value);
      local.onChange?.(newValues);
    } else {
      // Single value selection (radio, select)
      local.onChange?.(value);
    }
  };

  // Check if option is selected
  const isSelected = (optionValue) => {
    if (
      local.type === TYPES.MULTI_SELECT ||
      (local.type === TYPES.CHECKBOX && local.options.length > 0)
    ) {
      return Array.isArray(local.value) && local.value.includes(optionValue);
    }
    return local.value === optionValue;
  };

  const [isOpen, setIsOpen] = createSignal(false);

  // Render dropdown select
  const renderDropdownSelect = () => (
    <div class="relative">
      <select
        id={id()}
        name={name()}
        ref={local.ref}
        class={getSelectClasses()}
        disabled={local.disabled}
        aria-invalid={local.error ? 'true' : 'false'}
        value={local.value || ''}
        onChange={handleChange}
        {...others}
      >
        {local.placeholder && (
          <option value="" disabled>
            {local.placeholder}
          </option>
        )}
        <For each={local.options}>
          {(option) => (
            <option value={option.value} disabled={option.disabled}>
              {option.label}
            </option>
          )}
        </For>
      </select>
      <div class="absolute inset-y-0 right-0 flex items-center pr-2 pointer-events-none">
        {renderDropdownIcon()}
      </div>
    </div>
  );

  // Render radio group
  const renderRadioGroup = () => (
    <div class={getGroupClasses()} role="radiogroup">
      <For each={local.options}>
        {(option) => {
          const isDisabled = local.disabled || option.disabled;
          const optionId = `${id()}-${option.value}`;

          return (
            <div class="flex items-center">
              <input
                id={optionId}
                type="radio"
                name={name()}
                value={option.value}
                class={getInputClasses()}
                disabled={isDisabled}
                checked={isSelected(option.value)}
                onChange={handleChange}
                aria-invalid={local.error ? 'true' : 'false'}
              />
              <label for={optionId} class={`ml-2 ${getLabelClasses()}`}>
                {option.label}
              </label>
            </div>
          );
        }}
      </For>
    </div>
  );

  // Render checkbox (single or group)
  const renderCheckbox = () => {
    // Single checkbox without options
    if (local.options.length === 0) {
      return (
        <div class="flex items-start">
          <div class="flex items-center h-5">
            <input
              id={id()}
              type="checkbox"
              name={name()}
              ref={local.ref}
              class={getInputClasses()}
              disabled={local.disabled}
              checked={!!local.value}
              onChange={handleChange}
              aria-invalid={local.error ? 'true' : 'false'}
              {...others}
            />
          </div>
          {local.label && (
            <div class="ml-3 text-sm">
              <label for={id()} class={getLabelClasses()}>
                {local.label}
              </label>
              {local.description && (
                <p class="text-gray-500 dark:text-gray-400 text-xs mt-1">{local.description}</p>
              )}
            </div>
          )}
        </div>
      );
    }

    // Multiple checkboxes (for multi-select functionality)
    return (
      <div class={getGroupClasses()}>
        <For each={local.options}>
          {(option) => {
            const isDisabled = local.disabled || option.disabled;
            const optionId = `${id()}-${option.value}`;

            return (
              <div class="flex items-center">
                <input
                  id={optionId}
                  type="checkbox"
                  name={name()}
                  value={option.value}
                  class={getInputClasses()}
                  disabled={isDisabled}
                  checked={isSelected(option.value)}
                  onChange={handleChange}
                  aria-invalid={local.error ? 'true' : 'false'}
                />
                <label for={optionId} class={`ml-2 ${getLabelClasses()}`}>
                  {option.label}
                </label>
              </div>
            );
          }}
        </For>
      </div>
    );
  };

  // Render multi-select dropdown
  const renderMultiSelect = () => {
    const selectedCount = Array.isArray(local.value) ? local.value.length : 0;
    const displayText =
      selectedCount > 0 ? `${selectedCount} selected` : local.placeholder || 'Select options';

    return (
      <div class="relative">
        <button
          type="button"
          class={getSelectClasses()}
          disabled={local.disabled}
          aria-invalid={local.error ? 'true' : 'false'}
          onClick={() => !local.disabled && setIsOpen(!isOpen())}
        >
          {displayText}
        </button>
        <div class="absolute inset-y-0 right-0 flex items-center pr-2 pointer-events-none">
          {renderDropdownIcon()}
        </div>
        <Show when={isOpen() && !local.disabled}>
          <div class="absolute z-10 mt-1 w-full bg-white dark:bg-gray-900 shadow-lg max-h-60 rounded-md py-1 text-base ring-1 ring-black ring-opacity-5 overflow-auto focus:outline-none sm:text-sm border border-gray-300 dark:border-gray-600">
            <For each={local.options}>
              {(option) => {
                const isDisabled = option.disabled;

                return (
                  <div
                    class={`cursor-pointer select-none relative py-2 pl-3 pr-9 hover:bg-gray-100 dark:hover:bg-gray-800 ${isDisabled ? 'opacity-50 cursor-not-allowed' : ''}`}
                    onClick={() => {
                      if (!isDisabled) {
                        const currentValues = Array.isArray(local.value) ? local.value : [];
                        const isSelected = currentValues.includes(option.value);
                        const newValues = isSelected
                          ? currentValues.filter((v) => v !== option.value)
                          : [...currentValues, option.value];
                        local.onChange?.(newValues);
                      }
                    }}
                  >
                    <span
                      class={`block truncate ${isSelected(option.value) ? 'font-medium' : 'font-normal'}`}
                    >
                      {option.label}
                    </span>
                    <Show when={isSelected(option.value)}>
                      <span class="absolute inset-y-0 right-0 flex items-center pr-4 text-blue-600">
                        <svg class="h-5 w-5" viewBox="0 0 20 20" fill="currentColor">
                          <path
                            fill-rule="evenodd"
                            d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z"
                            clip-rule="evenodd"
                          />
                        </svg>
                      </span>
                    </Show>
                  </div>
                );
              }}
            </For>
          </div>
        </Show>
      </div>
    );
  };

  // Main render logic
  const renderContent = () => {
    switch (local.type) {
      case TYPES.RADIO:
        return renderRadioGroup();
      case TYPES.CHECKBOX:
        return renderCheckbox();
      case TYPES.MULTI_SELECT:
        return renderMultiSelect();
      case TYPES.SELECT:
      default:
        return renderDropdownSelect();
    }
  };

  // Wrapper for non-single-checkbox types
  const needsWrapper = () => {
    return !(local.type === TYPES.CHECKBOX && local.options.length === 0);
  };

  return (
    <Show when={needsWrapper()} fallback={renderContent()}>
      <div class={local.class}>
        {local.label && local.type !== TYPES.SELECT && (
          <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
            {local.label}
          </label>
        )}
        {renderContent()}
        {local.description && local.type !== TYPES.CHECKBOX && (
          <p class="mt-1 text-sm text-gray-500 dark:text-gray-400">{local.description}</p>
        )}
      </div>
    </Show>
  );
}

// Export constants for external use
Select.SIZES = SIZES;
Select.TYPES = TYPES;
Select.VARIANTS = VARIANTS;
Select.ORIENTATIONS = ORIENTATIONS;
