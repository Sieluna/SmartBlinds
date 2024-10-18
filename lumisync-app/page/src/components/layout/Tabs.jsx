import { createSignal, createEffect, For, splitProps, mergeProps } from 'solid-js';
import { Button } from '../ui/Button.jsx';

/**
 * Available tab style variants
 * @constant {Object}
 */
const VARIANTS = {
  DEFAULT: 'default',
  PILLS: 'pills',
  OUTLINE: 'outline',
  UNDERLINE: 'underline',
};

/**
 * Available tab size options
 * @constant {Object}
 */
const SIZES = {
  SM: 'sm',
  MD: 'md',
  LG: 'lg',
};

/**
 * Available tab alignment options
 * @constant {Object}
 */
const ALIGNMENTS = {
  LEFT: 'left',
  CENTER: 'center',
  RIGHT: 'right',
  BETWEEN: 'between',
  AROUND: 'around',
  EVENLY: 'evenly',
};

/**
 * Tabs component for creating tabbed navigation interfaces
 * @param {Object} props - Component properties
 */
export function Tabs(props) {
  const merged = mergeProps(
    {
      tabs: [],
      variant: VARIANTS.DEFAULT,
      size: SIZES.MD,
      align: ALIGNMENTS.LEFT,
      fullWidth: false,
      class: '',
    },
    props
  );

  const [local, others] = splitProps(merged, [
    'tabs',
    'activeTab',
    'onTabChange',
    'variant',
    'size',
    'align',
    'fullWidth',
    'class',
    'children',
  ]);

  // Initialize selected tab with proper reactivity
  const [selectedTab, setSelectedTab] = createSignal('');

  // Set initial tab with createEffect to handle reactive dependencies
  createEffect(() => {
    const initialTab = local.activeTab || (local.tabs.length > 0 ? local.tabs[0]?.id : '');
    if (initialTab && selectedTab() !== initialTab) {
      setSelectedTab(initialTab);
    }
  });

  const handleTabClick = (tabId) => {
    setSelectedTab(tabId);
    if (local.onTabChange) {
      local.onTabChange(tabId);
    }
  };

  const getTabsContainerClasses = () => {
    const alignClasses = {
      [ALIGNMENTS.LEFT]: 'justify-start',
      [ALIGNMENTS.CENTER]: 'justify-center',
      [ALIGNMENTS.RIGHT]: 'justify-end',
      [ALIGNMENTS.BETWEEN]: 'justify-between',
      [ALIGNMENTS.AROUND]: 'justify-around',
      [ALIGNMENTS.EVENLY]: 'justify-evenly',
    };

    return `flex overflow-x-auto space-x-1 pb-2 ${alignClasses[local.align]} ${local.fullWidth ? 'w-full' : ''} ${local.class}`;
  };

  const getTabButtonVariant = (tabId) => {
    const isActive = selectedTab() === tabId;

    switch (local.variant) {
      case VARIANTS.PILLS:
        return isActive ? Button.VARIANTS.PRIMARY : Button.VARIANTS.SECONDARY;
      case VARIANTS.OUTLINE:
        return isActive ? Button.VARIANTS.PRIMARY : Button.VARIANTS.GHOST;
      case VARIANTS.UNDERLINE:
        return Button.VARIANTS.GHOST;
      case VARIANTS.DEFAULT:
      default:
        return isActive ? Button.VARIANTS.PRIMARY : Button.VARIANTS.GHOST;
    }
  };

  const getTabButtonSize = () => {
    switch (local.size) {
      case SIZES.SM:
        return Button.SIZES.SM;
      case SIZES.LG:
        return Button.SIZES.LG;
      case SIZES.MD:
      default:
        return Button.SIZES.MD;
    }
  };

  const getTabButtonClass = (tabId) => {
    const isActive = selectedTab() === tabId;
    let classes = '';

    if (local.variant === VARIANTS.UNDERLINE && isActive) {
      classes += 'border-b-2 border-primary-500 -mb-px ';
    }

    if (local.fullWidth) {
      classes += 'flex-1 ';
    }

    return classes;
  };

  return (
    <div class="tabs-component" {...others}>
      <div class={getTabsContainerClasses()}>
        <For each={local.tabs}>
          {(tab) => (
            <Button
              variant={getTabButtonVariant(tab.id)}
              size={getTabButtonSize()}
              class={getTabButtonClass(tab.id)}
              onClick={() => handleTabClick(tab.id)}
            >
              {tab.label}
            </Button>
          )}
        </For>
      </div>

      <div class="tab-content mt-4">
        {typeof local.children === 'function' ? local.children(selectedTab()) : local.children}
      </div>
    </div>
  );
}

/**
 * TabPanel component for individual tab content
 * @param {Object} props - Component properties
 */
export function TabPanel(props) {
  const [local, others] = splitProps(props, ['id', 'activeTab', 'children', 'class']);

  return (
    <div
      class={`tab-panel ${local.activeTab === local.id ? 'block' : 'hidden'} ${local.class ?? ''}`}
      {...others}
    >
      {local.children}
    </div>
  );
}

// Export constants for external use
Tabs.VARIANTS = VARIANTS;
Tabs.SIZES = SIZES;
Tabs.ALIGNMENTS = ALIGNMENTS;
