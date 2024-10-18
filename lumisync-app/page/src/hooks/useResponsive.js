import { createSignal, createEffect, onCleanup } from 'solid-js';

/**
 * Breakpoint definitions matching common device sizes
 * @type {Object}
 */
const breakpoints = {
  xs: 0,
  sm: 640,
  md: 768,
  lg: 1024,
  xl: 1280,
  '2xl': 1536,
};

/**
 * Hook to track responsive breakpoints
 * @returns {Object} Responsive state and helper methods
 */
export function useResponsive() {
  const [windowSize, setWindowSize] = createSignal({
    width: typeof window !== 'undefined' ? window.innerWidth : 0,
    height: typeof window !== 'undefined' ? window.innerHeight : 0,
  });

  // Update window size on resize
  createEffect(() => {
    if (typeof window === 'undefined') return;

    const handleResize = () => {
      setWindowSize({
        width: window.innerWidth,
        height: window.innerHeight,
      });
    };

    window.addEventListener('resize', handleResize);

    onCleanup(() => {
      window.removeEventListener('resize', handleResize);
    });
  });

  /**
   * Check if current screen is larger than specified breakpoint
   * @param {string} breakpoint - Breakpoint name (xs, sm, md, lg, xl, 2xl)
   * @returns {boolean} True if screen width is greater than or equal to breakpoint
   */
  const isAbove = (breakpoint) => {
    const minWidth = breakpoints[breakpoint] || 0;
    return windowSize().width >= minWidth;
  };

  /**
   * Check if current screen is smaller than specified breakpoint
   * @param {string} breakpoint - Breakpoint name (xs, sm, md, lg, xl, 2xl)
   * @returns {boolean} True if screen width is less than breakpoint
   */
  const isBelow = (breakpoint) => {
    const breakpointValues = Object.entries(breakpoints);
    const index = breakpointValues.findIndex(([key]) => key === breakpoint);

    if (index === -1 || index + 1 >= breakpointValues.length) {
      return false;
    }

    const nextBreakpoint = breakpointValues[index + 1][1];
    return windowSize().width < nextBreakpoint;
  };

  /**
   * Check if current screen is between two breakpoints
   * @param {string} minBreakpoint - Minimum breakpoint name
   * @param {string} maxBreakpoint - Maximum breakpoint name
   * @returns {boolean} True if screen width is between specified breakpoints
   */
  const isBetween = (minBreakpoint, maxBreakpoint) => {
    return isAbove(minBreakpoint) && isBelow(maxBreakpoint);
  };

  /**
   * Get current active breakpoint
   * @returns {string} Current breakpoint name
   */
  const current = () => {
    const width = windowSize().width;
    const breakpointEntries = Object.entries(breakpoints).reverse();

    for (const [name, size] of breakpointEntries) {
      if (width >= size) return name;
    }

    return 'xs';
  };

  return {
    width: () => windowSize().width,
    height: () => windowSize().height,
    isAbove,
    isBelow,
    isBetween,
    current,
    breakpoints,
  };
}
