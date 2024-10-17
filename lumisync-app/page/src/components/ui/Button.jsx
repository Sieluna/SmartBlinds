import { splitProps } from 'solid-js';

/**
 * Button variants
 * @type {Object}
 */
const VARIANTS = {
  PRIMARY: 'primary',
  SECONDARY: 'secondary',
  GHOST: 'ghost',
  DANGER: 'danger',
};

/**
 * Button sizes
 * @type {Object}
 */
const SIZES = {
  XS: 'xs',
  SM: 'sm',
  MD: 'md',
  LG: 'lg',
  XL: 'xl',
};

/**
 * Button component with minimal variants and sizes
 * @param {Object} props - Component props
 */
export function Button(props) {
  const [local, others] = splitProps(props, [
    'children',
    'class',
    'variant',
    'size',
    'icon',
    'iconPosition',
    'fullWidth',
    'disabled',
  ]);

  const variant = () => local.variant ?? VARIANTS.PRIMARY;
  const size = () => local.size ?? SIZES.MD;
  const iconPosition = () => local.iconPosition ?? 'left';

  const getVariantClasses = () => {
    const baseClasses =
      'inline-flex items-center justify-center rounded-md transition-colors focus:outline-none focus:ring-2 focus:ring-opacity-50';

    const variantClasses = {
      [VARIANTS.PRIMARY]:
        'bg-black text-white hover:bg-gray-800 focus:ring-gray-500 active:bg-gray-900 dark:bg-white dark:text-black dark:hover:bg-gray-200 dark:active:bg-gray-300 dark:focus:ring-gray-400',
      [VARIANTS.SECONDARY]:
        'bg-gray-200 text-gray-900 hover:bg-gray-300 focus:ring-gray-400 active:bg-gray-400 dark:bg-gray-800 dark:text-gray-100 dark:hover:bg-gray-700 dark:active:bg-gray-600 dark:focus:ring-gray-500',
      [VARIANTS.GHOST]:
        'bg-transparent text-gray-900 hover:bg-gray-100 focus:ring-gray-300 active:bg-gray-200 dark:text-gray-100 dark:hover:bg-gray-800 dark:active:bg-gray-700 dark:focus:ring-gray-600',
      [VARIANTS.DANGER]:
        'bg-red-600 text-white hover:bg-red-700 focus:ring-red-500 active:bg-red-800 dark:bg-red-700 dark:hover:bg-red-800 dark:active:bg-red-900 dark:focus:ring-red-600',
    };

    const sizeClasses = {
      [SIZES.XS]: 'px-2 py-1 text-xs',
      [SIZES.SM]: 'px-3 py-1.5 text-sm',
      [SIZES.MD]: 'px-4 py-2 text-base',
      [SIZES.LG]: 'px-5 py-2.5 text-lg',
      [SIZES.XL]: 'px-6 py-3 text-xl',
    };

    const disabledClasses = local.disabled
      ? 'opacity-50 cursor-not-allowed pointer-events-none'
      : '';
    const fullWidthClasses = local.fullWidth ? 'w-full' : '';

    return `${baseClasses} ${variantClasses[variant()]} ${sizeClasses[size()]} ${disabledClasses} ${fullWidthClasses} ${local.class ?? ''}`;
  };

  const renderIcon = () => {
    if (!local.icon) return null;

    const iconSizeClasses = {
      [SIZES.XS]: 'h-3 w-3',
      [SIZES.SM]: 'h-4 w-4',
      [SIZES.MD]: 'h-5 w-5',
      [SIZES.LG]: 'h-6 w-6',
      [SIZES.XL]: 'h-7 w-7',
    };

    return (
      <span class={`${iconSizeClasses[size()]} ${iconPosition() === 'left' ? 'mr-2' : 'ml-2'}`}>
        {local.icon}
      </span>
    );
  };

  return (
    <button class={getVariantClasses()} disabled={local.disabled} {...others}>
      {iconPosition() === 'left' && renderIcon()}
      {local.children}
      {iconPosition() === 'right' && renderIcon()}
    </button>
  );
}

// Export constants for external use
Button.VARIANTS = VARIANTS;
Button.SIZES = SIZES;
