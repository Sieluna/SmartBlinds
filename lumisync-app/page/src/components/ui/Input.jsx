import { splitProps, mergeProps, createMemo, createUniqueId } from 'solid-js';

/**
 * Available input size options
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
 * Available input variants
 * @constant {Object}
 */
const VARIANTS = {
  DEFAULT: 'default',
  FILLED: 'filled',
  UNDERLINED: 'underlined',
};

/**
 * Input component for text input with validation states
 */
export function Input(props) {
  const merged = mergeProps(
    {
      type: 'text',
      size: SIZES.MD,
      variant: VARIANTS.DEFAULT,
    },
    props
  );

  const [local, others] = splitProps(merged, [
    'class',
    'size',
    'variant',
    'leftIcon',
    'rightIcon',
    'fullWidth',
    'error',
    'disabled',
    'ref',
    'id',
  ]);

  const inputId = createUniqueId();
  const id = () => local.id || inputId;

  const classes = createMemo(() => {
    const base = [
      'block',
      'w-full',
      'bg-white',
      'dark:bg-gray-900',
      'text-gray-900',
      'dark:text-gray-100',
      'placeholder-gray-500',
      'dark:placeholder-gray-400',
      'focus:outline-none',
      'transition-colors',
      'disabled:opacity-50',
      'disabled:cursor-not-allowed',
    ];

    const sizes = {
      [SIZES.XS]: ['px-2', 'py-1', 'text-xs'],
      [SIZES.SM]: ['px-2.5', 'py-1.5', 'text-sm'],
      [SIZES.MD]: ['px-3', 'py-2', 'text-sm'],
      [SIZES.LG]: ['px-4', 'py-2.5', 'text-base'],
      [SIZES.XL]: ['px-5', 'py-3', 'text-lg'],
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
      ? [
          'border-red-300',
          'dark:border-red-600',
          'text-red-900',
          'dark:text-red-100',
          'focus:ring-red-500',
          'focus:border-red-500',
        ]
      : [
          'focus:ring-blue-500',
          'focus:border-blue-500',
          'dark:focus:ring-blue-400',
          'dark:focus:border-blue-400',
        ];

    const iconPadding = [];
    if (local.leftIcon) {
      const leftPadding = {
        [SIZES.XS]: 'pl-7',
        [SIZES.SM]: 'pl-8',
        [SIZES.MD]: 'pl-10',
        [SIZES.LG]: 'pl-12',
        [SIZES.XL]: 'pl-14',
      };
      iconPadding.push(leftPadding[local.size]);
    }

    if (local.rightIcon) {
      const rightPadding = {
        [SIZES.XS]: 'pr-7',
        [SIZES.SM]: 'pr-8',
        [SIZES.MD]: 'pr-10',
        [SIZES.LG]: 'pr-12',
        [SIZES.XL]: 'pr-14',
      };
      iconPadding.push(rightPadding[local.size]);
    }

    return [
      ...base,
      ...sizes[local.size],
      ...variants[local.variant],
      ...states,
      ...iconPadding,
      local.class,
    ]
      .filter(Boolean)
      .join(' ');
  });

  const iconClasses = createMemo(() => {
    const iconSizes = {
      [SIZES.XS]: 'h-3 w-3',
      [SIZES.SM]: 'h-4 w-4',
      [SIZES.MD]: 'h-5 w-5',
      [SIZES.LG]: 'h-6 w-6',
      [SIZES.XL]: 'h-7 w-7',
    };
    return iconSizes[local.size];
  });

  return (
    <div class="relative">
      {local.leftIcon && (
        <div class="absolute inset-y-0 left-0 pl-2.5 flex items-center pointer-events-none">
          <span class={iconClasses()}>{local.leftIcon}</span>
        </div>
      )}

      <input
        id={id()}
        ref={local.ref}
        class={classes()}
        disabled={local.disabled}
        aria-invalid={local.error ? 'true' : 'false'}
        {...others}
      />

      {local.rightIcon && (
        <div class="absolute inset-y-0 right-0 pr-2.5 flex items-center pointer-events-none">
          <span class={iconClasses()}>{local.rightIcon}</span>
        </div>
      )}
    </div>
  );
}

// Export constants for external use
Input.SIZES = SIZES;
Input.VARIANTS = VARIANTS;
