import { splitProps, createContext, useContext } from 'solid-js';
import { createStore } from 'solid-js/store';

/**
 * Form context to manage form state and actions
 */
const FormContext = createContext();

/**
 * Form component for creating and managing forms
 * @param {Object} props - Component props
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
    const hasErrors = Object.values(formState.errors).some(error => error !== undefined);
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
  const handleSubmit = async event => {
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
