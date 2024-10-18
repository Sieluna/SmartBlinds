import {
  splitProps,
  createEffect,
  createContext,
  useContext,
  children as resolveChildren,
  Show,
} from 'solid-js';
import { createStore } from 'solid-js/store';

/**
 * Form context to manage form state and actions
 */
const FormContext = createContext();

/**
 * Form component for creating and managing forms with validation and state management
 * @param {Object} props - Component properties
 * @param {Object} [props.initialValues={}] - Initial form values
 * @param {Object} [props.validator] - Object containing validation functions for each field
 * @param {Function} [props.onSubmit] - Form submission handler
 * @param {string} [props.class=''] - Additional CSS classes
 * @param {JSXElement} props.children - Form content
 * @returns {JSXElement} Rendered form component
 */
export function Form(props) {
  const [local, others] = splitProps(props, [
    'children',
    'class',
    'onSubmit',
    'initialValues',
    'validator',
  ]);

  const [formState, setFormState] = createStore({
    values: local.initialValues ?? {},
    errors: {},
    touched: {},
    isSubmitting: false,
    isValid: true,
  });

  /**
   * Set a field value
   * @param {string} name - Field name
   * @param {any} value - Field value
   */
  const setFieldValue = (name, value) => {
    setFormState('values', name, value);
    setFormState('touched', name, true);

    // Validate field if validator is provided
    if (local.validator && local.validator[name]) {
      const error = local.validator[name](value, formState.values);
      setFormState('errors', name, error);
    }

    // Update isValid state
    updateValidState();
  };

  /**
   * Set a field error
   * @param {string} name - Field name
   * @param {string} error - Error message
   */
  const setFieldError = (name, error) => {
    setFormState('errors', name, error);
    updateValidState();
  };

  /**
   * Set touched state for a field
   * @param {string} name - Field name
   * @param {boolean} isTouched - Whether field is touched
   */
  const setFieldTouched = (name, isTouched = true) => {
    setFormState('touched', name, isTouched);
  };

  /**
   * Update the form validity state
   */
  const updateValidState = () => {
    const hasErrors = Object.values(formState.errors).some((error) => error !== undefined);
    setFormState('isValid', !hasErrors);
  };

  /**
   * Reset the form to initial values
   */
  const resetForm = () => {
    setFormState({
      values: local.initialValues ?? {},
      errors: {},
      touched: {},
      isSubmitting: false,
      isValid: true,
    });
  };

  /**
   * Handle form submission
   * @param {Event} event - Form submit event
   */
  const handleSubmit = async (event) => {
    event.preventDefault();

    setFormState('isSubmitting', true);

    // Run validation if validator is provided
    if (local.validator) {
      const newErrors = {};
      let hasErrors = false;

      Object.entries(local.validator).forEach(([field, validator]) => {
        const error = validator(formState.values[field], formState.values);
        if (error) {
          newErrors[field] = error;
          hasErrors = true;
        }
      });

      setFormState('errors', newErrors);
      setFormState('isValid', !hasErrors);

      if (hasErrors) {
        setFormState('isSubmitting', false);
        return;
      }
    }

    // Call onSubmit handler if provided
    if (local.onSubmit) {
      try {
        await local.onSubmit(formState.values, {
          setFieldError,
          resetForm,
          setFormState,
        });
      } catch (error) {
        console.error('Form submission error:', error);
      }
    }

    setFormState('isSubmitting', false);
  };

  const formContext = {
    values: () => formState.values,
    errors: () => formState.errors,
    touched: () => formState.touched,
    isSubmitting: () => formState.isSubmitting,
    isValid: () => formState.isValid,
    setFieldValue,
    setFieldError,
    setFieldTouched,
    resetForm,
  };

  return (
    <FormContext.Provider value={formContext}>
      <form class={`${local.class ?? ''}`} onSubmit={handleSubmit} novalidate {...others}>
        {local.children}
      </form>
    </FormContext.Provider>
  );
}

/**
 * Custom hook for using form context
 * @returns {Object} Form context value
 */
export function useForm() {
  const context = useContext(FormContext);
  if (!context) {
    throw new Error('useForm must be used within a Form component');
  }
  return context;
}

/**
 * FormField component for managing individual form fields with validation and error handling
 * @param {Object} props - Component properties
 * @param {string} props.name - Field name
 * @param {string} [props.label] - Field label
 * @param {string} [props.hint] - Helper text
 * @param {boolean} [props.required=false] - Whether field is required
 * @param {Function} [props.onChange] - Change event handler
 * @param {Function} [props.onBlur] - Blur event handler
 * @param {any} [props.value] - Field value
 * @param {string} [props.error] - Error message
 * @param {string} [props.class=''] - Additional CSS classes
 * @param {JSXElement|Function} props.children - Field content or render function
 * @returns {JSXElement} Rendered form field component
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
  const handleChange = (event) => {
    const value = event.target.type === 'checkbox' ? event.target.checked : event.target.value;
    form.setFieldValue(local.name, value);
    local.onChange?.(event);
  };

  /**
   * Handle field blur event
   * @param {Event} event - Blur event
   */
  const handleBlur = (event) => {
    form.setFieldTouched(local.name, true);
    local.onBlur?.(event);
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

    return childrenArray.map((child) => {
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
