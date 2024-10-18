import { splitProps, mergeProps, createMemo } from 'solid-js';

/**
 * Available button style variants
 * @constant {Object}
 */
const VARIANTS = {
  PRIMARY: 'primary',
  SECONDARY: 'secondary',
  GHOST: 'ghost',
  DANGER: 'danger',
};

/**
 * Available button size options
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
 * Button component with customizable styles and sizes
 */
export function Button(props) {
  const merged = mergeProps(
    {
      variant: VARIANTS.PRIMARY,
      size: SIZES.MD,
      iconPosition: 'left',
      loading: false,
    },
    props
  );

  const [local, others] = splitProps(merged, [
    'children',
    'class',
    'variant',
    'size',
    'icon',
    'iconPosition',
    'fullWidth',
    'loading',
    'disabled',
    'ref',
  ]);

  const isDisabled = () => local.disabled || local.loading;

  const classes = createMemo(() => {
    const base = [
      'inline-flex',
      'items-center',
      'justify-center',
      'rounded-md',
      'font-medium',
      'transition-colors',
      'focus:outline-none',
      'focus:ring-2',
      'focus:ring-offset-2',
    ];

    const variants = {
      [VARIANTS.PRIMARY]: [
        'bg-blue-600',
        'text-white',
        'hover:bg-blue-700',
        'focus:ring-blue-500',
        'dark:bg-blue-700',
        'dark:hover:bg-blue-800',
      ],
      [VARIANTS.SECONDARY]: [
        'bg-gray-200',
        'text-gray-900',
        'hover:bg-gray-300',
        'focus:ring-gray-500',
        'dark:bg-gray-700',
        'dark:text-gray-100',
        'dark:hover:bg-gray-600',
      ],
      [VARIANTS.GHOST]: [
        'bg-transparent',
        'text-gray-700',
        'hover:bg-gray-100',
        'focus:ring-gray-500',
        'dark:text-gray-300',
        'dark:hover:bg-gray-800',
      ],
      [VARIANTS.DANGER]: [
        'bg-red-600',
        'text-white',
        'hover:bg-red-700',
        'focus:ring-red-500',
        'dark:bg-red-700',
        'dark:hover:bg-red-800',
      ],
    };

    const sizes = {
      [SIZES.XS]: ['px-2', 'py-1', 'text-xs'],
      [SIZES.SM]: ['px-3', 'py-1.5', 'text-sm'],
      [SIZES.MD]: ['px-4', 'py-2', 'text-sm'],
      [SIZES.LG]: ['px-5', 'py-2.5', 'text-base'],
      [SIZES.XL]: ['px-6', 'py-3', 'text-lg'],
    };

    const modifiers = [];

    if (isDisabled()) {
      modifiers.push('opacity-50', 'cursor-not-allowed', 'pointer-events-none');
    }

    if (local.fullWidth) {
      modifiers.push('w-full');
    }

    return [...base, ...variants[local.variant], ...sizes[local.size], ...modifiers, local.class]
      .filter(Boolean)
      .join(' ');
  });

  const iconClasses = createMemo(() => {
    const iconSizes = {
      [SIZES.XS]: 'h-3 w-3',
      [SIZES.SM]: 'h-4 w-4',
      [SIZES.MD]: 'h-4 w-4',
      [SIZES.LG]: 'h-5 w-5',
      [SIZES.XL]: 'h-6 w-6',
    };

    const spacing = local.iconPosition === 'left' ? 'mr-2' : 'ml-2';
    return `${iconSizes[local.size]} ${spacing}`;
  });

  const renderIcon = () => {
    if (local.loading) {
      return (
        <svg class={`${iconClasses()} animate-spin`} fill="none" viewBox="0 0 24 24">
          <circle
            class="opacity-25"
            cx="12"
            cy="12"
            r="10"
            stroke="currentColor"
            stroke-width="4"
          />
          <path
            class="opacity-75"
            fill="currentColor"
            d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
          />
        </svg>
      );
    }

    if (local.icon) {
      return <span class={iconClasses()}>{local.icon}</span>;
    }

    return null;
  };

  return (
    <button ref={local.ref} class={classes()} disabled={isDisabled()} {...others}>
      {local.iconPosition === 'left' && renderIcon()}
      {local.children}
      {local.iconPosition === 'right' && renderIcon()}
    </button>
  );
}

// Export constants for external use
Button.VARIANTS = VARIANTS;
Button.SIZES = SIZES;
