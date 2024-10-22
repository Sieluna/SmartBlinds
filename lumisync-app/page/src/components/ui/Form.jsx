import {
  createContext,
  useContext,
  createSignal,
  createMemo,
  createEffect,
  batch,
  untrack,
  splitProps,
  mergeProps,
  Switch,
  Match,
  Show,
} from 'solid-js';
import { createStore, produce } from 'solid-js/store';
import { Input } from './Input.jsx';
import { Button } from './Button.jsx';
import { Select } from './Select.jsx';

/**
 * Available form size options
 * @constant {Object}
 */
const SIZES = {
  SM: 'sm',
  MD: 'md',
  LG: 'lg',
};

/**
 * Available form style variants
 * @constant {Object}
 */
const VARIANTS = {
  DEFAULT: 'default',
  COMPACT: 'compact',
  CARD: 'card',
};

/**
 * Available validation rule types
 * @constant {Object}
 */
const VALIDATION_TYPES = {
  REQUIRED: 'required',
  EMAIL: 'email',
  MIN_LENGTH: 'minLength',
  MAX_LENGTH: 'maxLength',
  PATTERN: 'pattern',
  CUSTOM: 'custom',
};

/**
 * Form context for state management
 */
const FormContext = createContext();

/**
 * Hook to access form context
 * @returns {Object} Form context
 */
export function useForm() {
  const context = useContext(FormContext);
  if (!context) {
    throw new Error('useForm must be used within a Form component');
  }
  return context;
}

/**
 * Create form store with validation and submission logic
 * @param {Object} initialValues - Initial form values
 * @param {Object} validationRules - Validation rules
 * @returns {Object} Form store
 */
export function createFormStore(initialValues = {}, validationRules = {}) {
  const [state, setState] = createStore({
    values: initialValues,
    errors: {},
    touched: {},
  });

  const [isSubmitting, setIsSubmitting] = createSignal(false);
  const [submitCount, setSubmitCount] = createSignal(0);

  const validateField = (name, value, rules) => {
    if (!rules) return null;

    for (const rule of rules) {
      let isValid = true;
      let message = '';

      switch (rule.type) {
        case VALIDATION_TYPES.REQUIRED:
          isValid = value !== undefined && value !== null && value !== '';
          message = rule.message || `${name} is required`;
          break;

        case VALIDATION_TYPES.EMAIL:
          isValid = !value || /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value);
          message = rule.message || 'Please enter a valid email address';
          break;

        case VALIDATION_TYPES.MIN_LENGTH:
          isValid = !value || value.length >= rule.value;
          message = rule.message || `Must be at least ${rule.value} characters`;
          break;

        case VALIDATION_TYPES.MAX_LENGTH:
          isValid = !value || value.length <= rule.value;
          message = rule.message || `Must be no more than ${rule.value} characters`;
          break;

        case VALIDATION_TYPES.PATTERN:
          isValid = !value || rule.value.test(value);
          message = rule.message || 'Invalid format';
          break;

        case VALIDATION_TYPES.CUSTOM:
          try {
            isValid = rule.validator(value, state.values);
            message = rule.message || 'Invalid value';
          } catch (error) {
            isValid = false;
            message = error.message || 'Validation error';
          }
          break;

        default:
          break;
      }

      if (!isValid) {
        return message;
      }
    }

    return null;
  };

  /**
   * Validate entire form
   * @returns {boolean} Whether form is valid
   */
  const validateForm = () => {
    const newErrors = {};
    const { values } = state;

    for (const [name, rules] of Object.entries(validationRules)) {
      const error = validateField(name, values[name], rules);
      if (error) {
        newErrors[name] = error;
      }
    }

    setState('errors', newErrors);
    return Object.keys(newErrors).length === 0;
  };

  /**
   * Set field value and clear error
   * @param {string} name - Field name
   * @param {*} value - Field value
   */
  const setValue = (name, value) => {
    batch(() => {
      const path = name.split('.');
      setState('values', ...path, value);
      setState(
        'errors',
        produce((errors) => {
          delete errors[name];
        })
      );
    });
  };

  /**
   * Mark field as touched
   * @param {string} name - Field name
   * @param {boolean} isTouched - Whether field is touched
   */
  const setFieldTouched = (name, isTouched = true) => {
    setState('touched', name, isTouched);
  };

  /**
   * Set field error
   * @param {string} name - Field name
   * @param {string|null} error - Error message
   */
  const setFieldError = (name, error) => {
    if (error == null) {
      setState(
        'errors',
        produce((errors) => {
          delete errors[name];
        })
      );
    } else {
      setState('errors', name, error);
    }
  };

  /**
   * Reset form to initial state
   */
  const reset = () => {
    batch(() => {
      setState('values', initialValues);
      setState('errors', {});
      setState('touched', {});
      setSubmitCount(0);
    });
  };

  // Computed values
  const isValid = createMemo(() => Object.keys(state.errors).length === 0);
  const isDirty = createMemo(() => {
    const { values } = state;
    return Object.keys(values).some((key) => values[key] !== initialValues[key]);
  });

  return {
    values: () => state.values,
    errors: () => state.errors,
    touched: () => state.touched,
    isSubmitting,
    submitCount,
    isValid,
    isDirty,
    setValue,
    setFieldTouched,
    setFieldError,
    setIsSubmitting,
    setSubmitCount,
    validateField,
    validateForm,
    reset,
  };
}

/**
 * Create field controller for shared field logic
 * @param {string} name - Field name
 * @param {Array} rules - Validation rules
 * @param {Object} options - Field options
 * @returns {Object} Field controller
 */
function createFieldController(name, rules, options) {
  const { validateOnBlur, validateOnChange } = options;
  const form = useForm();

  const value = () => {
    const path = name.split('.');
    let current = form.values();
    for (const key of path) {
      current = current?.[key];
    }
    return current ?? '';
  };
  const error = () => form.errors()[name];
  const touched = () => form.touched()[name];

  const handleChange = (newValue) => {
    let processedValue = newValue;

    if (newValue && typeof newValue === 'object' && newValue.target) {
      processedValue = newValue.target.value;
    }

    if (processedValue === undefined || processedValue === null) {
      processedValue = '';
    }

    form.setValue(name, processedValue);

    if (validateOnChange && rules) {
      const error = form.validateField(name, processedValue, rules);
      form.setFieldError(name, error);
    }
  };

  const handleBlur = () => {
    form.setFieldTouched(name, true);

    if (validateOnBlur && rules) {
      const error = form.validateField(name, value(), rules);
      form.setFieldError(name, error);
    }
  };

  // Auto-validate when field was touched or form was submitted
  createEffect(() => {
    const currentValue = value();
    const hasBeenTouched = touched();
    const hasBeenSubmitted = form.submitCount() > 0;

    if ((hasBeenTouched || hasBeenSubmitted) && rules) {
      const error = form.validateField(name, currentValue, rules);
      form.setFieldError(name, error);
    }
  });

  return {
    value,
    error,
    touched,
    change: handleChange,
    blur: handleBlur,
  };
}

/**
 * Form field wrapper component
 * @param {Object} props - Component properties
 */
export function FormField(props) {
  const merged = mergeProps(
    {
      component: 'input',
      validateOnBlur: true,
      validateOnChange: false,
    },
    props
  );

  const [local, others] = splitProps(merged, [
    'name',
    'label',
    'description',
    'component',
    'children',
    'validateOnBlur',
    'validateOnChange',
    'rules',
    'class',
  ]);

  const controller = createFieldController(local.name, local.rules, {
    validateOnBlur: local.validateOnBlur,
    validateOnChange: local.validateOnChange,
  });

  const form = useForm();

  const isRequired = () => local.rules?.some((rule) => rule.type === VALIDATION_TYPES.REQUIRED);
  const maxLengthRule = () =>
    local.rules?.find((rule) => rule.type === VALIDATION_TYPES.MAX_LENGTH);
  const shouldShowError = () =>
    controller.error() && (controller.touched() || form.submitCount() > 0);

  const fieldProps = () => ({
    value: controller.value(),
    error: controller.error(),
    onBlur: controller.blur,
    onChange: controller.change,
    onInput: (e) => controller.change(e.target.value),
    ...others,
  });

  return (
    <div class={`form-field ${local.class || ''}`}>
      {/* Label with required indicator */}
      <Show when={local.label}>
        <label
          for={local.name}
          class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2"
        >
          {local.label}
          <Show when={isRequired()}>
            <span class="text-red-500 ml-1" aria-label="required">
              *
            </span>
          </Show>
        </label>
      </Show>

      {/* Field renderer */}
      <Switch>
        <Match when={local.children}>
          {typeof local.children === 'function' ? local.children(fieldProps()) : local.children}
        </Match>
        <Match when={local.component === 'input'}>
          <Input id={local.name} name={local.name} {...fieldProps()} />
        </Match>
        <Match when={local.component === 'select'}>
          <Select id={local.name} name={local.name} {...fieldProps()} />
        </Match>
        <Match when={true}>
          <div class="text-red-500 text-sm">Unsupported component type: {local.component}</div>
        </Match>
      </Switch>

      {/* Description text */}
      <Show when={local.description && !shouldShowError()}>
        <p class="mt-1 text-sm text-gray-500 dark:text-gray-400">{local.description}</p>
      </Show>

      {/* Error message with icon */}
      <Show when={shouldShowError()}>
        <p class="mt-1 text-sm text-red-600 dark:text-red-400 flex items-center">
          <svg
            class="h-4 w-4 mr-1 flex-shrink-0"
            fill="currentColor"
            viewBox="0 0 20 20"
            aria-hidden="true"
          >
            <path
              fill-rule="evenodd"
              d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7 4a1 1 0 11-2 0 1 1 0 012 0zm-1-9a1 1 0 00-1 1v4a1 1 0 102 0V6a1 1 0 00-1-1z"
              clip-rule="evenodd"
            />
          </svg>
          {controller.error()}
        </p>
      </Show>

      {/* Character count for maxLength validation */}
      <Show
        when={maxLengthRule() && ['text', 'textarea', 'email', 'password'].includes(others.type)}
      >
        {(() => {
          const rule = maxLengthRule();
          const currentLength = String(controller.value()).length;
          const maxLength = rule.value;
          const isNearLimit = currentLength > maxLength * 0.8;
          const isOverLimit = currentLength > maxLength;

          return (
            <p
              class={`mt-1 text-xs text-right ${
                isOverLimit
                  ? 'text-red-600 dark:text-red-400'
                  : isNearLimit
                    ? 'text-orange-600 dark:text-orange-400'
                    : 'text-gray-500 dark:text-gray-400'
              }`}
            >
              {currentLength}/{maxLength}
            </p>
          );
        })()}
      </Show>
    </div>
  );
}

/**
 * Enhanced Input component that integrates with Form
 * @param {Object} props - Component properties
 */
export function FormInput(props) {
  return <FormField {...props} component="input" />;
}

/**
 * Enhanced Select component that integrates with Form
 * @param {Object} props - Component properties
 */
export function FormSelect(props) {
  return <FormField {...props} component="select" />;
}

/**
 * Form submit button component
 * @param {Object} props - Component properties
 */
export function FormSubmitButton(props) {
  const merged = mergeProps(
    {
      variant: Button.VARIANTS.PRIMARY,
      size: Button.SIZES.MD,
      loadingText: 'Submitting...',
    },
    props
  );

  const [local, others] = splitProps(merged, ['children', 'loadingText']);
  const form = useForm();

  return (
    <Button
      type="submit"
      variant={merged.variant}
      size={merged.size}
      loading={form.isSubmitting()}
      disabled={!form.isValid() || form.isSubmitting()}
      {...others}
    >
      {form.isSubmitting() && local.loadingText ? local.loadingText : local.children}
    </Button>
  );
}

/**
 * Form reset button component
 * @param {Object} props - Component properties
 */
export function FormResetButton(props) {
  const merged = mergeProps(
    {
      variant: Button.VARIANTS.SECONDARY,
      size: Button.SIZES.MD,
    },
    props
  );

  const [local, others] = splitProps(merged, ['children']);
  const form = useForm();

  return (
    <Button
      type="button"
      variant={merged.variant}
      size={merged.size}
      onClick={form.reset}
      disabled={form.isSubmitting() || !form.isDirty()}
      {...others}
    >
      {local.children || 'Reset'}
    </Button>
  );
}

/**
 * Main Form component with validation and submission logic
 * @param {Object} props - Component properties
 */
export function Form(props) {
  const merged = mergeProps(
    {
      size: SIZES.MD,
      variant: VARIANTS.DEFAULT,
      noValidate: true,
    },
    props
  );

  const [local, others] = splitProps(merged, [
    'children',
    'class',
    'size',
    'variant',
    'formStore',
    'onSubmit',
    'initialValues',
    'validationRules',
  ]);

  // Create form store if not provided
  const formStore = untrack(
    () => local.formStore || createFormStore(local.initialValues, local.validationRules)
  );

  const formClasses = createMemo(() => {
    const base = ['form'];

    const sizes = {
      [SIZES.SM]: ['space-y-3'],
      [SIZES.MD]: ['space-y-4'],
      [SIZES.LG]: ['space-y-6'],
    };

    const variants = {
      [VARIANTS.DEFAULT]: [],
      [VARIANTS.COMPACT]: ['space-y-2'],
      [VARIANTS.CARD]: [
        'bg-white',
        'dark:bg-gray-900',
        'p-6',
        'rounded-lg',
        'border',
        'border-gray-200',
        'dark:border-gray-700',
        'shadow-sm',
      ],
    };

    return [...base, ...sizes[local.size], ...variants[local.variant], local.class]
      .filter(Boolean)
      .join(' ');
  });

  const handleSubmit = async (event) => {
    event.preventDefault();

    formStore.setIsSubmitting(true);
    formStore.setSubmitCount((count) => count + 1);

    try {
      const isValid = formStore.validateForm();

      if (isValid && local.onSubmit) {
        await local.onSubmit(formStore.values(), formStore);
      }
    } catch (error) {
      console.error('Form submission error:', error);
    } finally {
      formStore.setIsSubmitting(false);
    }
  };

  return (
    <FormContext.Provider value={formStore}>
      <form
        class={formClasses()}
        onSubmit={handleSubmit}
        noValidate={merged.noValidate}
        {...others}
      >
        {local.children}
      </form>
    </FormContext.Provider>
  );
}

// Export constants for external use
Form.SIZES = SIZES;
Form.VARIANTS = VARIANTS;
Form.VALIDATION_TYPES = VALIDATION_TYPES;
Form.Field = FormField;
Form.Input = FormInput;
Form.Select = FormSelect;
Form.SubmitButton = FormSubmitButton;
Form.ResetButton = FormResetButton;
Form.useForm = useForm;
Form.createFormStore = createFormStore;
