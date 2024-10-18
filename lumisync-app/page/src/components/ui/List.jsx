import { splitProps, mergeProps, createMemo, For, Show } from 'solid-js';
import { Dynamic } from 'solid-js/web';

/**
 * Available list types
 * @constant {Object}
 */
const TYPES = {
  UNORDERED: 'ul',
  ORDERED: 'ol',
  DESCRIPTION: 'dl',
  SIMPLE: 'simple',
};

/**
 * Available list size options
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
 * Available list style variants
 * @constant {Object}
 */
const VARIANTS = {
  DEFAULT: 'default',
  BORDERED: 'bordered',
  DIVIDED: 'divided',
  FLUSH: 'flush',
  CARD: 'card',
};

/**
 * Available list orientations
 * @constant {Object}
 */
const ORIENTATIONS = {
  VERTICAL: 'vertical',
  HORIZONTAL: 'horizontal',
};

/**
 * List component for displaying structured data in various formats
 * @param {Object} props - Component properties
 */
export function List(props) {
  const merged = mergeProps(
    {
      type: TYPES.SIMPLE,
      size: SIZES.MD,
      variant: VARIANTS.DEFAULT,
      orientation: ORIENTATIONS.VERTICAL,
      items: [],
      hoverable: false,
      selectable: false,
    },
    props
  );

  const [local, others] = splitProps(merged, [
    'items',
    'type',
    'size',
    'variant',
    'orientation',
    'hoverable',
    'selectable',
    'selectedValue',
    'onItemClick',
    'onItemSelect',
    'renderItem',
    'class',
    'children',
  ]);

  const containerClasses = createMemo(() => {
    const base = ['list-none'];

    const variants = {
      [VARIANTS.DEFAULT]: [],
      [VARIANTS.BORDERED]: [
        'border',
        'border-gray-200',
        'dark:border-gray-700',
        'rounded-lg',
        'overflow-hidden',
      ],
      [VARIANTS.DIVIDED]: ['divide-y', 'divide-gray-200', 'dark:divide-gray-700'],
      [VARIANTS.FLUSH]: [],
      [VARIANTS.CARD]: [
        'bg-white',
        'dark:bg-gray-800',
        'shadow',
        'rounded-lg',
        'overflow-hidden',
        'border',
        'border-gray-200',
        'dark:border-gray-700',
      ],
    };

    const orientations = {
      [ORIENTATIONS.VERTICAL]: ['space-y-0'],
      [ORIENTATIONS.HORIZONTAL]: ['flex', 'flex-wrap', 'gap-2'],
    };

    return [...base, ...variants[local.variant], ...orientations[local.orientation], local.class]
      .filter(Boolean)
      .join(' ');
  });

  const itemClasses = createMemo(() => {
    const base = ['flex', 'items-center', 'transition-colors'];

    const sizes = {
      [SIZES.XS]: ['px-2', 'py-1', 'text-xs'],
      [SIZES.SM]: ['px-3', 'py-2', 'text-sm'],
      [SIZES.MD]: ['px-4', 'py-3', 'text-sm'],
      [SIZES.LG]: ['px-5', 'py-4', 'text-base'],
      [SIZES.XL]: ['px-6', 'py-5', 'text-lg'],
    };

    const interactions = [];
    if (local.hoverable || local.selectable || local.onItemClick) {
      interactions.push(
        'cursor-pointer',
        'hover:bg-gray-50',
        'dark:hover:bg-gray-800',
        'focus:outline-none',
        'focus:bg-gray-50',
        'dark:focus:bg-gray-800'
      );
    }

    const variants = {
      [VARIANTS.DEFAULT]: [],
      [VARIANTS.BORDERED]: [
        'border-b',
        'border-gray-200',
        'dark:border-gray-700',
        'last:border-b-0',
      ],
      [VARIANTS.DIVIDED]: [],
      [VARIANTS.FLUSH]: [],
      [VARIANTS.CARD]: ['border-b', 'border-gray-200', 'dark:border-gray-700', 'last:border-b-0'],
    };

    const orientations = {
      [ORIENTATIONS.VERTICAL]: ['w-full'],
      [ORIENTATIONS.HORIZONTAL]: ['flex-shrink-0'],
    };

    return [
      ...base,
      ...sizes[local.size],
      ...interactions,
      ...variants[local.variant],
      ...orientations[local.orientation],
    ]
      .filter(Boolean)
      .join(' ');
  });

  const selectedItemClasses = createMemo(() => {
    return [
      'bg-blue-50',
      'dark:bg-blue-900/20',
      'border-blue-200',
      'dark:border-blue-800',
      'text-blue-900',
      'dark:text-blue-100',
    ].join(' ');
  });

  const iconClasses = createMemo(() => {
    const iconSizes = {
      [SIZES.XS]: 'h-3 w-3',
      [SIZES.SM]: 'h-4 w-4',
      [SIZES.MD]: 'h-5 w-5',
      [SIZES.LG]: 'h-6 w-6',
      [SIZES.XL]: 'h-7 w-7',
    };
    return `${iconSizes[local.size]} mr-3 flex-shrink-0`;
  });

  const avatarClasses = createMemo(() => {
    const avatarSizes = {
      [SIZES.XS]: 'h-6 w-6',
      [SIZES.SM]: 'h-8 w-8',
      [SIZES.MD]: 'h-10 w-10',
      [SIZES.LG]: 'h-12 w-12',
      [SIZES.XL]: 'h-16 w-16',
    };
    return `${avatarSizes[local.size]} mr-3 flex-shrink-0 rounded-full bg-gray-200 dark:bg-gray-700`;
  });

  const handleItemClick = (item, index) => {
    if (item.disabled) return;

    if (local.selectable && local.onItemSelect) {
      local.onItemSelect(item.value !== undefined ? item.value : item, index);
    }

    if (local.onItemClick) {
      local.onItemClick(item, index);
    }

    if (item.onClick) {
      item.onClick(item, index);
    }
  };

  const isSelected = (item) => {
    if (!local.selectable) return false;
    const itemValue = item.value !== undefined ? item.value : item;
    return local.selectedValue === itemValue;
  };

  const renderIcon = (item) => {
    if (item.avatar) {
      return (
        <div class={avatarClasses()}>
          <Show
            when={typeof item.avatar === 'string'}
            fallback={
              <div class="h-full w-full flex items-center justify-center">{item.avatar}</div>
            }
          >
            <img src={item.avatar} alt="" class="h-full w-full object-cover rounded-full" />
          </Show>
        </div>
      );
    }

    if (item.icon) {
      return <div class={iconClasses()}>{item.icon}</div>;
    }

    return null;
  };

  const renderItemContent = (item, index) => {
    if (local.renderItem) {
      return local.renderItem(item, index, { isSelected: isSelected(item) });
    }

    // Support both simple strings/elements and structured item objects
    if (typeof item === 'string' || !item || typeof item !== 'object') {
      return <span class="flex-1">{item}</span>;
    }

    return (
      <>
        {renderIcon(item)}
        <div class="flex-1 min-w-0">
          <div class="flex items-center justify-between">
            <div class="min-w-0 flex-1">
              {item.title && (
                <p class="font-medium text-gray-900 dark:text-gray-100 truncate">{item.title}</p>
              )}
              {item.content && (
                <p class="text-gray-900 dark:text-gray-100 truncate">{item.content}</p>
              )}
              {item.description && (
                <p class="text-gray-500 dark:text-gray-400 text-sm truncate mt-1">
                  {item.description}
                </p>
              )}
            </div>
            {item.action && <div class="ml-3 flex-shrink-0">{item.action}</div>}
          </div>
        </div>
      </>
    );
  };

  const getListTag = () => {
    switch (local.type) {
      case TYPES.ORDERED:
        return 'ol';
      case TYPES.UNORDERED:
        return 'ul';
      case TYPES.DESCRIPTION:
        return 'dl';
      case TYPES.SIMPLE:
      default:
        return 'div';
    }
  };

  return (
    <Dynamic
      component={getListTag()}
      class={containerClasses()}
      role={local.selectable ? 'listbox' : 'list'}
      aria-orientation={local.orientation}
      {...others}
    >
      <Show
        when={local.children}
        fallback={
          <For each={local.items}>
            {(item, index) => {
              const isItemSelected = isSelected(item);
              const isDisabled = item && typeof item === 'object' && item.disabled;

              const itemClass = [
                itemClasses(),
                isItemSelected ? selectedItemClasses() : '',
                isDisabled ? 'opacity-50 cursor-not-allowed' : '',
              ]
                .filter(Boolean)
                .join(' ');

              const content = (
                <div
                  class={itemClass}
                  onClick={() => !isDisabled && handleItemClick(item, index)}
                  role={local.selectable ? 'option' : local.onItemClick ? 'button' : undefined}
                  aria-selected={local.selectable ? isItemSelected : undefined}
                  aria-disabled={isDisabled}
                  tabindex={
                    local.hoverable || local.selectable || local.onItemClick ? 0 : undefined
                  }
                >
                  {renderItemContent(item, index)}
                </div>
              );

              if (
                local.type === TYPES.DESCRIPTION &&
                item &&
                typeof item === 'object' &&
                item.term
              ) {
                return (
                  <>
                    <dt class="font-medium text-gray-900 dark:text-gray-100">{item.term}</dt>
                    <dd class="text-gray-500 dark:text-gray-400 ml-0">
                      {item.description || item.content}
                    </dd>
                  </>
                );
              }

              return local.type === TYPES.SIMPLE ? content : <li>{content}</li>;
            }}
          </For>
        }
      >
        {local.children}
      </Show>
    </Dynamic>
  );
}

/**
 * ListItem component for manual list construction
 * @param {Object} props - Component properties
 */
export function ListItem(props) {
  const [local, others] = splitProps(props, ['children', 'class']);

  return (
    <li class={local.class} {...others}>
      {local.children}
    </li>
  );
}

// Export constants for external use
List.TYPES = TYPES;
List.SIZES = SIZES;
List.VARIANTS = VARIANTS;
List.ORIENTATIONS = ORIENTATIONS;
