import { splitProps } from 'solid-js';

/**
 * Container sizes
 * @type {Object}
 */
const SIZES = {
  SM: 'sm',
  MD: 'md',
  LG: 'lg',
  XL: 'xl',
  FULL: 'full',
  AUTO: 'auto',
};

/**
 * Container component for wrapping content with responsive width
 * @param {Object} props - Component props
 */
export function Container(props) {
  const [local, others] = splitProps(props, ['children', 'class', 'size', 'padding', 'fluid']);

  const size = () => local.size ?? SIZES.LG;
  const padding = () => local.padding ?? true;

  const getContainerClasses = () => {
    const baseClasses = 'mx-auto';

    const paddingClasses = padding() ? 'px-4 sm:px-6 lg:px-8' : '';

    const sizeClasses = {
      [SIZES.SM]: 'max-w-3xl',
      [SIZES.MD]: 'max-w-4xl',
      [SIZES.LG]: 'max-w-6xl',
      [SIZES.XL]: 'max-w-7xl',
      [SIZES.FULL]: 'max-w-full',
      [SIZES.AUTO]: '',
    };

    const fluidClasses = local.fluid ? 'w-full' : '';

    return `${baseClasses} ${sizeClasses[size()]} ${paddingClasses} ${fluidClasses} ${local.class ?? ''}`;
  };

  return (
    <div class={getContainerClasses()} {...others}>
      {local.children}
    </div>
  );
}

// Export constants for external use
Container.SIZES = SIZES;
