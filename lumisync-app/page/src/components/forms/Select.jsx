import { splitProps, mergeProps, For } from 'solid-js';

/**
 * Select component for selecting from a list of options
 * @param {Object} props - Component props
 */
export function Select(props) {
  const defaultProps = {
    class: '',
    options: [],
  };

  const merged = mergeProps(defaultProps, props);

  const [local, others] = splitProps(merged, [
    'class',
    'options',
    'placeholder',
    'error',
    'disabled',
    'fullWidth',
    'icon',
  ]);

  const getSelectClasses = () => {
    const baseClasses =
      'block w-full px-3 py-2 pr-10 bg-white dark:bg-gray-800 border border-gray-300 dark:border-gray-700 rounded-md shadow-sm focus:outline-none sm:text-sm appearance-none';
    const errorClasses = local.error
      ? 'border-red-300 text-red-900 focus:ring-red-500 focus:border-red-500'
      : 'focus:ring-blue-500 focus:border-blue-500 dark:focus:ring-blue-400 dark:focus:border-blue-400';
    const disabledClasses = local.disabled
      ? 'bg-gray-100 dark:bg-gray-900 text-gray-500 cursor-not-allowed'
      : '';
    const widthClasses = local.fullWidth ? 'w-full' : '';

    return `${baseClasses} ${errorClasses} ${disabledClasses} ${widthClasses} ${local.class}`;
  };

  return (
    <div class="relative w-full">
      <select class={getSelectClasses()} disabled={local.disabled} {...others}>
        {local.placeholder && (
          <option value="" class="text-gray-400" disabled selected={!props.value}>
            {local.placeholder}
          </option>
        )}
        <For each={local.options}>
          {option => (
            <option
              value={option.value}
              selected={props.value === option.value}
              disabled={option.disabled}
            >
              {option.label}
            </option>
          )}
        </For>
      </select>

      <div class="absolute inset-y-0 right-0 flex items-center pr-2 pointer-events-none">
        {local.icon ?? (
          <svg
            class="h-5 w-5 text-gray-400"
            xmlns="http://www.w3.org/2000/svg"
            viewBox="0 0 20 20"
            fill="currentColor"
            aria-hidden="true"
          >
            <path
              fill-rule="evenodd"
              d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z"
              clip-rule="evenodd"
            />
          </svg>
        )}
      </div>
    </div>
  );
}
