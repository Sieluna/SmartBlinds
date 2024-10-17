import { splitProps, mergeProps, For } from 'solid-js';

/**
 * Radio component for selecting a single option from multiple choices
 * @param {Object} props - Component props
 */
export function Radio(props) {
  const defaultProps = {
    class: '',
    options: [],
  };

  const merged = mergeProps(defaultProps, props);

  const [local, others] = splitProps(merged, ['class', 'name', 'options', 'error', 'disabled']);

  const getRadioClasses = () => {
    const baseClasses =
      'h-4 w-4 border-gray-300 text-blue-600 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-700 dark:ring-offset-gray-800 dark:focus:ring-blue-400';
    const errorClasses = local.error ? 'border-red-300 focus:ring-red-500' : '';
    const disabledClasses = local.disabled ? 'opacity-50 cursor-not-allowed' : '';

    return `${baseClasses} ${errorClasses} ${disabledClasses}`;
  };

  return (
    <div class={`space-y-2 ${local.class}`}>
      <For each={local.options}>
        {option => (
          <div class="flex items-center">
            <input
              type="radio"
              id={`${local.name}-${option.value}`}
              name={local.name}
              value={option.value}
              class={getRadioClasses()}
              disabled={local.disabled || option.disabled}
              {...others}
            />
            {option.label && (
              <label
                for={`${local.name}-${option.value}`}
                class="ml-2 text-sm text-gray-700 dark:text-gray-300"
              >
                {option.label}
              </label>
            )}
          </div>
        )}
      </For>
    </div>
  );
}
