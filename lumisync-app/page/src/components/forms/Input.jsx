import { splitProps, mergeProps } from 'solid-js';

/**
 * Input component for collecting user input
 * @param {Object} props - Component props
 */
export function Input(props) {
  const defaultProps = {
    type: 'text',
    class: '',
  };

  const merged = mergeProps(defaultProps, props);

  const [local, others] = splitProps(merged, [
    'class',
    'type',
    'placeholder',
    'error',
    'disabled',
    'leftIcon',
    'rightIcon',
    'fullWidth',
  ]);

  const getInputClasses = () => {
    const baseClasses =
      'block w-full px-3 py-2 bg-white dark:bg-gray-800 border border-gray-300 dark:border-gray-700 rounded-md shadow-sm focus:outline-none sm:text-sm';
    const errorClasses = local.error
      ? 'border-red-300 text-red-900 placeholder-red-300 focus:ring-red-500 focus:border-red-500'
      : 'focus:ring-blue-500 focus:border-blue-500 dark:focus:ring-blue-400 dark:focus:border-blue-400';
    const disabledClasses = local.disabled
      ? 'bg-gray-100 dark:bg-gray-900 text-gray-500 cursor-not-allowed'
      : '';
    const widthClasses = local.fullWidth ? 'w-full' : '';
    const leftPadding = local.leftIcon ? 'pl-10' : '';
    const rightPadding = local.rightIcon ? 'pr-10' : '';

    return `${baseClasses} ${errorClasses} ${disabledClasses} ${widthClasses} ${leftPadding} ${rightPadding} ${local.class}`;
  };

  return (
    <div class="relative w-full">
      {local.leftIcon && (
        <div class="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none">
          {local.leftIcon}
        </div>
      )}

      <input
        type={local.type}
        class={getInputClasses()}
        placeholder={local.placeholder}
        disabled={local.disabled}
        {...others}
      />

      {local.rightIcon && (
        <div class="absolute inset-y-0 right-0 pr-3 flex items-center pointer-events-none">
          {local.rightIcon}
        </div>
      )}
    </div>
  );
}
