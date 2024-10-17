import { splitProps, createEffect, children as resolveChildren } from 'solid-js';
import { Show } from 'solid-js';

import { useForm } from './Form.jsx';

/**
 * FormField component for managing individual form fields
 * @param {Object} props - Component props
 */
export function FormField(props) {
  const [local, others] = splitProps(props, [
    'children',
    'class',
    'name',
    'label',
    'hint',
    'required',
    'onChange',
    'onBlur',
    'value',
    'error',
  ]);

  const form = useForm();

  // Update form value when component value changes
  createEffect(() => {
    if (local.value !== undefined) {
      form.setFieldValue(local.name, local.value);
    }
  });

  // Sync with externally provided error
  createEffect(() => {
    if (local.error !== undefined) {
      form.setFieldError(local.name, local.error);
    }
  });

  /**
   * Handle field change event
   * @param {Event} event - Change event
   */
  const handleChange = event => {
    const value = event.target.type === 'checkbox' ? event.target.checked : event.target.value;

    form.setFieldValue(local.name, value);

    // Call custom onChange handler if provided
    if (local.onChange) {
      local.onChange(event);
    }
  };

  /**
   * Handle field blur event
   * @param {Event} event - Blur event
   */
  const handleBlur = event => {
    form.setFieldTouched(local.name, true);

    // Call custom onBlur handler if provided
    if (local.onBlur) {
      local.onBlur(event);
    }
  };

  /**
   * Get field value from form state or local props
   * @returns {any} Field value
   */
  const getValue = () => {
    // Priority: local prop value > form value
    return local.value !== undefined ? local.value : form.values()[local.name];
  };

  /**
   * Check if field has an error
   * @returns {boolean} Whether field has error
   */
  const hasError = () => {
    return !!form.errors()[local.name] && form.touched()[local.name];
  };

  /**
   * Get field error message
   * @returns {string|undefined} Error message
   */
  const getErrorMessage = () => {
    return hasError() ? form.errors()[local.name] : undefined;
  };

  /**
   * Check if field is a checkbox or radio
   * @returns {boolean} Whether field is a checkbox or radio
   */
  const isCheckboxOrRadio = () => {
    const childrenArray = Array.isArray(local.children) ? local.children : [local.children];
    for (const child of childrenArray) {
      if (child && typeof child === 'object') {
        if (
          child.type &&
          (child.type.name === 'Checkbox' ||
            child.type.name === 'Radio' ||
            child.type === 'checkbox' ||
            child.type === 'radio')
        ) {
          return true;
        }

        if (
          child.props &&
          child.props.type &&
          (child.props.type === 'checkbox' || child.props.type === 'radio')
        ) {
          return true;
        }
      }
    }
    return false;
  };

  // Build props for child input component
  const getInputProps = (childProps = {}) => {
    return {
      id: `field-${local.name}`,
      name: local.name,
      value: getValue() || '',
      checked:
        childProps.type === 'checkbox' || childProps.type === 'radio' ? !!getValue() : undefined,
      'aria-invalid': hasError() ? 'true' : 'false',
      'aria-describedby': hasError()
        ? `${local.name}-error`
        : local.hint
          ? `${local.name}-hint`
          : undefined,
      onChange: handleChange,
      onBlur: handleBlur,
      required: local.required,
      ...childProps,
    };
  };

  // Inject form props into children
  const resolvedChildren = resolveChildren(() => {
    const childrenArray = Array.isArray(local.children) ? local.children : [local.children];

    return childrenArray.map(child => {
      if (typeof child === 'function' && child.length > 0) {
        return child(getInputProps);
      }

      if (child && typeof child === 'object' && child.type) {
        const componentName = child.type.name;

        if (componentName === 'Checkbox' || componentName === 'Radio') {
          const childProps = {
            ...child.props,
            onChange: handleChange,
            onBlur: handleBlur,
            checked: !!getValue(),
            error: hasError() ? getErrorMessage() : undefined,
          };

          return {
            ...child,
            props: childProps,
          };
        }
      }

      return child;
    });
  });

  return (
    <Show 
      when={isCheckboxOrRadio()}
      fallback={
        <div class={`mb-4 ${local.class ?? ''}`} {...others}>
          {local.label && (
            <label
              for={`field-${local.name}`}
              class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
            >
              {local.label}
              {local.required && <span class="text-red-500 ml-1">*</span>}
            </label>
          )}

          {resolvedChildren()}

          {local.hint && !hasError() && (
            <p id={`${local.name}-hint`} class="mt-1 text-sm text-gray-500 dark:text-gray-400">
              {local.hint}
            </p>
          )}

          {hasError() && (
            <p id={`${local.name}-error`} class="mt-1 text-sm text-red-600 dark:text-red-400">
              {getErrorMessage()}
            </p>
          )}
        </div>
      }
    >
      <div class={`mb-4 ${local.class ?? ''}`} {...others}>
        <div class="flex items-start">
          <div class="flex items-center h-5">{resolvedChildren()}</div>

          {local.label && (
            <div class="ml-3 text-sm">
              <label
                for={`field-${local.name}`}
                class="font-medium text-gray-700 dark:text-gray-300"
              >
                {local.label}
                {local.required && <span class="text-red-500 ml-1">*</span>}
              </label>
            </div>
          )}
        </div>

        {local.hint && !hasError() && (
          <p id={`${local.name}-hint`} class="mt-1 ml-7 text-sm text-gray-500 dark:text-gray-400">
            {local.hint}
          </p>
        )}

        {hasError() && (
          <p id={`${local.name}-error`} class="mt-1 ml-7 text-sm text-red-600 dark:text-red-400">
            {getErrorMessage()}
          </p>
        )}
      </div>
    </Show>
  );
}
