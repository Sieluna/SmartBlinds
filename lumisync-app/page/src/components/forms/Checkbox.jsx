import { splitProps, mergeProps } from 'solid-js';

/**
 * Checkbox component for boolean input
 * @param {Object} props - Component props
 */
export function Checkbox(props) {
  const defaultProps = {
    class: '',
  };

  const merged = mergeProps(defaultProps, props);

  const [local, others] = splitProps(merged, ['class', 'label', 'error', 'disabled']);

  const getCheckboxClasses = () => {
    const baseClasses =
      'h-4 w-4 rounded border-gray-300 text-blue-600 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-700 dark:ring-offset-gray-800 dark:focus:ring-blue-400';
    const errorClasses = local.error ? 'border-red-300 focus:ring-red-500' : '';
    const disabledClasses = local.disabled ? 'opacity-50 cursor-not-allowed' : '';

    return `${baseClasses} ${errorClasses} ${disabledClasses} ${local.class}`;
  };

  return (
    <div class="flex items-center">
      <input type="checkbox" class={getCheckboxClasses()} disabled={local.disabled} {...others} />
      {local.label && (
        <span class="ml-2 text-sm text-gray-700 dark:text-gray-300">{local.label}</span>
      )}
    </div>
  );
}
