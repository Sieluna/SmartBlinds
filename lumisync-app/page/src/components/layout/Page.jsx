import { splitProps } from 'solid-js';
import { Container } from './Container.jsx';

/**
 * Page component for consistent page layouts
 * @param {Object} props - Component props
 */
export function Page(props) {
  const [local, others] = splitProps(props, [
    'children',
    'class',
    'title',
    'description',
    'containerSize',
    'containerClass',
    'fullWidth',
    'noPadding',
    'header',
    'footer',
  ]);

  return (
    <div
      class={`min-h-screen bg-gray-50 dark:bg-gray-900 ${local.noPadding ? '' : 'py-8'} ${local.class ?? ''}`}
      {...others}
    >
      {local.header}

      <Container
        size={local.containerSize ?? (local.fullWidth ? 'full' : 'lg')}
        class={local.containerClass}
        padding={!local.noPadding}
      >
        {(local.title || local.description) && (
          <div class="mb-6">
            {local.title && (
              <h1 class="text-2xl font-bold text-gray-900 dark:text-white">{local.title}</h1>
            )}
            {local.description && (
              <p class="mt-1 text-sm text-gray-600 dark:text-gray-400">{local.description}</p>
            )}
          </div>
        )}

        {local.children}
      </Container>

      {local.footer}
    </div>
  );
}

/**
 * PageHeader component for consistent page headers
 * @param {Object} props - Component props
 */
export function PageHeader(props) {
  const [local, others] = splitProps(props, [
    'children',
    'class',
    'title',
    'description',
    'actions',
    'breadcrumbs',
    'containerSize',
    'containerClass',
    'fullWidth',
    'border',
    'spacing',
  ]);

  const getSpacingClasses = () => {
    const spacings = {
      sm: 'py-4',
      md: 'py-6',
      lg: 'py-8',
      xl: 'py-10',
      none: '',
    };

    return spacings[local.spacing ?? 'md'];
  };

  return (
    <header
      class={`bg-white dark:bg-gray-800 shadow-sm ${local.border ? 'border-b border-gray-200 dark:border-gray-700' : ''} ${getSpacingClasses()} ${local.class ?? ''}`}
      {...others}
    >
      <Container
        size={local.containerSize ?? (local.fullWidth ? 'full' : 'lg')}
        class={local.containerClass}
      >
        {local.breadcrumbs && <div class="mb-4">{local.breadcrumbs}</div>}

        <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between">
          <div>
            {local.title && (
              <h1 class="text-2xl font-bold text-gray-900 dark:text-white">{local.title}</h1>
            )}
            {local.description && (
              <p class="mt-1 text-sm text-gray-600 dark:text-gray-400">{local.description}</p>
            )}
            {local.children}
          </div>

          {local.actions && <div class="mt-4 flex sm:mt-0 sm:ml-4">{local.actions}</div>}
        </div>
      </Container>
    </header>
  );
}

/**
 * PageContent component for page content area
 * @param {Object} props - Component props
 */
export function PageContent(props) {
  const [local, others] = splitProps(props, ['children', 'class', 'padding']);

  const padding = () => (local.padding !== undefined ? local.padding : true);

  return (
    <div class={`${padding() ? 'py-6' : ''} ${local.class ?? ''}`} {...others}>
      {local.children}
    </div>
  );
}

/**
 * PageFooter component for consistent page footers
 * @param {Object} props - Component props
 */
export function PageFooter(props) {
  const [local, others] = splitProps(props, [
    'children',
    'class',
    'containerSize',
    'containerClass',
    'fullWidth',
    'border',
    'spacing',
  ]);

  const getSpacingClasses = () => {
    const spacings = {
      sm: 'py-4',
      md: 'py-6',
      lg: 'py-8',
      xl: 'py-10',
      none: '',
    };

    return spacings[local.spacing ?? 'md'];
  };

  return (
    <footer
      class={`bg-white dark:bg-gray-800 ${local.border ? 'border-t border-gray-200 dark:border-gray-700' : ''} ${getSpacingClasses()} ${local.class ?? ''}`}
      {...others}
    >
      <Container
        size={local.containerSize ?? (local.fullWidth ? 'full' : 'lg')}
        class={local.containerClass}
      >
        {local.children}
      </Container>
    </footer>
  );
}
