import { splitProps, mergeProps, createMemo, createUniqueId } from 'solid-js';

/**
 * Available checkbox size options
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
 * Available label position options
 * @constant {Object}
 */
const LABEL_POSITIONS = {
  LEFT: 'left',
  RIGHT: 'right',
};

/**
 * Checkbox component for boolean input selection with customizable styles
 * @param {Object} props - Component properties
 */
export function Checkbox(props) {
  const merged = mergeProps({ size: SIZES.MD }, props);

  const [local, others] = splitProps(merged, [
    'class',
    'size',
    'label',
    'description',
    'error',
    'disabled',
    'ref',
    'id',
  ]);

  const checkboxId = createUniqueId();
  const id = () => local.id || checkboxId;

  const classes = createMemo(() => {
    const base = [
      'rounded',
      'border-gray-300',
      'dark:border-gray-600',
      'text-blue-600',
      'dark:bg-gray-800',
      'focus:ring-blue-500',
      'dark:focus:ring-blue-400',
      'focus:ring-2',
      'focus:ring-offset-2',
      'transition-colors',
    ];

    const sizes = {
      [SIZES.XS]: ['h-3', 'w-3'],
      [SIZES.SM]: ['h-4', 'w-4'],
      [SIZES.MD]: ['h-4', 'w-4'],
      [SIZES.LG]: ['h-5', 'w-5'],
      [SIZES.XL]: ['h-6', 'w-6'],
    };

    const states = [];
    if (local.error) {
      states.push('border-red-300', 'dark:border-red-600');
    }
    if (local.disabled) {
      states.push('opacity-50', 'cursor-not-allowed');
    }

    return [...base, ...sizes[local.size], ...states, local.class].filter(Boolean).join(' ');
  });

  const labelClasses = createMemo(() => {
    const base = ['select-none'];

    const sizes = {
      [SIZES.XS]: ['text-xs'],
      [SIZES.SM]: ['text-sm'],
      [SIZES.MD]: ['text-sm'],
      [SIZES.LG]: ['text-base'],
      [SIZES.XL]: ['text-lg'],
    };

    const states = [];
    if (local.disabled) {
      states.push('text-gray-400', 'cursor-not-allowed');
    } else {
      states.push('text-gray-700', 'dark:text-gray-300', 'cursor-pointer');
    }

    return [...base, ...sizes[local.size], ...states].filter(Boolean).join(' ');
  });

  return (
    <div class="flex items-start">
      <div class="flex items-center h-5">
        <input
          id={id()}
          type="checkbox"
          ref={local.ref}
          class={classes()}
          disabled={local.disabled}
          aria-invalid={local.error ? 'true' : 'false'}
          {...others}
        />
      </div>

      {local.label && (
        <div class="ml-3 text-sm">
          <label for={id()} class={labelClasses()}>
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

// Export constants for external use
Checkbox.SIZES = SIZES;
Checkbox.LABEL_POSITIONS = LABEL_POSITIONS;
