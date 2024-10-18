import { createSignal, Show } from 'solid-js';
import { Button } from '../components/ui/Button.jsx';
import { Input } from '../components/ui/Input.jsx';
import { Select } from '../components/ui/Select.jsx';
import { Form } from '../components/ui/Form.jsx';
import { Tabs, TabPanel } from '../components/layout/Tabs.jsx';
import { Page, PageContent } from '../components/layout/Page.jsx';
import { Container } from '../components/layout/Container.jsx';
import { List } from '../components/ui/List.jsx';

export function Debug() {
  // Current active tab
  const [activeTab, setActiveTab] = createSignal('button');

  // Button testing signals
  const [buttonSize, setButtonSize] = createSignal(Button.SIZES.MD);
  const [buttonVariant, setButtonVariant] = createSignal(Button.VARIANTS.PRIMARY);
  const [buttonLoading, setButtonLoading] = createSignal(false);
  const [buttonDisabled, setButtonDisabled] = createSignal(false);
  const [buttonFullWidth, setButtonFullWidth] = createSignal(false);
  const [buttonShowIcon, setButtonShowIcon] = createSignal(true);
  const [buttonIconPosition, setButtonIconPosition] = createSignal('left');

  // Input testing signals
  const [inputSize, setInputSize] = createSignal(Input.SIZES.MD);
  const [inputVariant, setInputVariant] = createSignal(Input.VARIANTS.DEFAULT);
  const [inputValue, setInputValue] = createSignal('');
  const [inputError, setInputError] = createSignal(false);
  const [inputDisabled, setInputDisabled] = createSignal(false);
  const [inputShowLeftIcon, setInputShowLeftIcon] = createSignal(true);
  const [inputShowRightIcon, setInputShowRightIcon] = createSignal(false);
  const [inputFullWidth, setInputFullWidth] = createSignal(true);

  // Select testing signals
  const [selectType, setSelectType] = createSignal(Select.TYPES.SELECT);
  const [selectSize, setSelectSize] = createSignal(Select.SIZES.MD);
  const [selectVariant, setSelectVariant] = createSignal(Select.VARIANTS.DEFAULT);
  const [selectOrientation, setSelectOrientation] = createSignal(Select.ORIENTATIONS.VERTICAL);
  const [selectValue, setSelectValue] = createSignal('');
  const [selectError, setSelectError] = createSignal(false);
  const [selectDisabled, setSelectDisabled] = createSignal(false);

  // List testing signals
  const [listType, setListType] = createSignal(List.TYPES.SIMPLE);
  const [listSize, setListSize] = createSignal(List.SIZES.MD);
  const [listVariant, setListVariant] = createSignal(List.VARIANTS.DEFAULT);
  const [listOrientation, setListOrientation] = createSignal(List.ORIENTATIONS.VERTICAL);
  const [listHoverable, setListHoverable] = createSignal(true);
  const [listSelectable, setListSelectable] = createSignal(true);
  const [listSelectedValue, setListSelectedValue] = createSignal('');

  // Form testing signals
  const [formSize, setFormSize] = createSignal(Form.SIZES.MD);
  const [formVariant, setFormVariant] = createSignal(Form.VARIANTS.DEFAULT);
  const [formSubmitMessage, setFormSubmitMessage] = createSignal('');

  // Common test options
  const testOptions = [
    { label: 'First Option', value: 'first' },
    { label: 'Second Option', value: 'second' },
    { label: 'Third Option', value: 'third' },
    { label: 'Fourth Option', value: 'fourth' },
  ];

  // Icons
  const SearchIcon = () => (
    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor">
      <path
        fillRule="evenodd"
        d="M9 3.5a5.5 5.5 0 100 11 5.5 5.5 0 000-11zM2 9a7 7 0 1112.452 4.391l3.328 3.329a.75.75 0 11-1.06 1.06l-3.329-3.328A7 7 0 012 9z"
        clipRule="evenodd"
      />
    </svg>
  );

  const EyeIcon = () => (
    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor">
      <path d="M10 12a2 2 0 100-4 2 2 0 000 4z" />
      <path
        fill-rule="evenodd"
        clip-rule="evenodd"
        d="M.458 10C1.732 5.943 5.522 3 10 3s8.268 2.943 9.542 7c-1.274 4.057-5.064 7-9.542 7S1.732 14.057.458 10zM14 10a4 4 0 11-8 0 4 4 0 018 0z"
      />
    </svg>
  );

  const HeartIcon = () => (
    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor">
      <path d="M9.653 16.915l-.005-.003-.019-.01a20.759 20.759 0 01-1.162-.682 22.045 22.045 0 01-2.582-1.9C4.045 12.733 2 10.352 2 7.5a4.5 4.5 0 018-2.828A4.5 4.5 0 0118 7.5c0 2.852-2.045 5.233-3.885 6.82a22.049 22.049 0 01-3.744 2.582l-.019.01-.005.003h-.002a.739.739 0 01-.69.001l-.002-.001z" />
    </svg>
  );

  // List items
  const listItems = [
    {
      title: 'Test 1',
      description: 'Test description 1',
      value: 'test',
      avatar: <SearchIcon />,
      action: (
        <Button size="xs" variant="ghost">
          Edit
        </Button>
      ),
    },
    {
      title: 'Test 2',
      description: 'Test description 2',
      value: 'test',
      avatar: <SearchIcon />,
      action: (
        <Button size="xs" variant="ghost">
          Edit
        </Button>
      ),
    },
    {
      title: 'Test 3',
      description: 'Test description 3',
      value: 'test',
      avatar: <SearchIcon />,
      disabled: true,
      action: (
        <Button size="xs" variant="ghost" disabled>
          Edit
        </Button>
      ),
    },
  ];

  const simpleListItems = ['First Item', 'Second Item', 'Third Item', 'Fourth Item'];

  const descriptionItems = [
    { term: 'Name', content: 'Test 1' },
    { term: 'Email', content: 'test@example.com' },
  ];

  // Form initial values and validation rules
  const formInitialValues = {
    name: '',
    email: '',
    password: '',
    confirmPassword: '',
    role: '',
    agreeToTerms: false,
  };

  const formValidationRules = {
    name: [
      {
        type: Form.VALIDATION_TYPES.REQUIRED,
        message: 'Name is required',
      },
      {
        type: Form.VALIDATION_TYPES.MIN_LENGTH,
        value: 2,
        message: 'Name must be at least 2 characters',
      },
      {
        type: Form.VALIDATION_TYPES.MAX_LENGTH,
        value: 50,
        message: 'Name cannot exceed 50 characters',
      },
    ],
    email: [
      {
        type: Form.VALIDATION_TYPES.REQUIRED,
        message: 'Email is required',
      },
      {
        type: Form.VALIDATION_TYPES.EMAIL,
        message: 'Please enter a valid email address',
      },
    ],
    password: [
      {
        type: Form.VALIDATION_TYPES.REQUIRED,
        message: 'Password is required',
      },
      {
        type: Form.VALIDATION_TYPES.MIN_LENGTH,
        value: 6,
        message: 'Password must be at least 6 characters',
      },
      {
        type: Form.VALIDATION_TYPES.PATTERN,
        value: /^(?=.*[a-z])(?=.*[A-Z])(?=.*\d)/,
        message:
          'Password must contain at least one uppercase letter, one lowercase letter, and one number',
      },
    ],
    confirmPassword: [
      {
        type: Form.VALIDATION_TYPES.REQUIRED,
        message: 'Please confirm your password',
      },
      {
        type: Form.VALIDATION_TYPES.CUSTOM,
        validator: (value, values) => value === values.password,
        message: 'Passwords do not match',
      },
    ],
    role: [
      {
        type: Form.VALIDATION_TYPES.REQUIRED,
        message: 'Please select a role',
      },
    ],
    agreeToTerms: [
      {
        type: Form.VALIDATION_TYPES.CUSTOM,
        validator: (value) => value === true,
        message: 'You must agree to the terms and conditions',
      },
    ],
  };

  const roleOptions = [
    { label: 'Admin', value: 'admin' },
    { label: 'User', value: 'user' },
  ];

  // Tab definitions
  const tabs = [
    { id: 'button', label: 'Button' },
    { id: 'input', label: 'Input' },
    { id: 'select', label: 'Select' },
    { id: 'list', label: 'List' },
    { id: 'form', label: 'Form' },
  ];

  // Control Panel Component
  const ControlPanel = (props) => (
    <div class="w-full lg:w-80 bg-gray-50 dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700 p-4">
      <h3 class="text-lg font-semibold mb-4 text-gray-900 dark:text-gray-100">
        {props.title} Controls
      </h3>
      <div class="space-y-4">{props.children}</div>
    </div>
  );

  // Preview Panel Component
  const PreviewPanel = (props) => (
    <div class="flex-1 flex flex-col items-center justify-center">
      <div class="w-full max-w-md bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700 p-6">
        <h3 class="text-lg font-semibold mb-4 text-center text-gray-900 dark:text-gray-100">
          {props.title}
        </h3>
        <div class="flex flex-col items-center space-y-4">{props.children}</div>
      </div>
      <div class="mt-4 max-w-md w-full">
        <div class="text-xs text-gray-500 p-3 bg-gray-50 dark:bg-gray-800 rounded border border-gray-200 dark:border-gray-700">
          <div class="font-medium mb-1">Current Settings:</div>
          <div class="whitespace-pre-wrap">{props.info}</div>
        </div>
      </div>
    </div>
  );

  // Component Layout
  const ComponentLayout = (props) => (
    <div class="flex flex-col lg:flex-row gap-6 min-h-[500px]">
      <ControlPanel title={props.title}>{props.controls}</ControlPanel>
      <PreviewPanel title={`${props.title} Preview`} info={props.info}>
        {props.preview}
      </PreviewPanel>
    </div>
  );

  return (
    <Page title="Component Debug">
      <PageContent>
        <Container class="max-w-7xl">
          <Tabs
            tabs={tabs}
            activeTab={activeTab()}
            onTabChange={setActiveTab}
            variant={Tabs.VARIANTS.UNDERLINE}
            size={Tabs.SIZES.LG}
            align={Tabs.ALIGNMENTS.CENTER}
            class="mb-8"
          >
            {/* Button Tab */}
            <TabPanel id="button" activeTab={activeTab()}>
              <ComponentLayout
                title="Button"
                controls={
                  <>
                    <Select
                      type={Select.TYPES.SELECT}
                      label="Size"
                      size="sm"
                      placeholder="Select size"
                      options={Object.values(Button.SIZES).map((size) => ({
                        label: size.toUpperCase(),
                        value: size,
                      }))}
                      value={buttonSize()}
                      onChange={setButtonSize}
                    />
                    <Select
                      type={Select.TYPES.SELECT}
                      label="Variant"
                      size="sm"
                      placeholder="Select variant"
                      options={Object.values(Button.VARIANTS).map((variant) => ({
                        label: variant.charAt(0).toUpperCase() + variant.slice(1),
                        value: variant,
                      }))}
                      value={buttonVariant()}
                      onChange={setButtonVariant}
                    />
                    <Select
                      type={Select.TYPES.SELECT}
                      label="Icon Position"
                      size="sm"
                      placeholder="Select position"
                      options={[
                        { label: 'Left', value: 'left' },
                        { label: 'Right', value: 'right' },
                      ]}
                      value={buttonIconPosition()}
                      onChange={setButtonIconPosition}
                    />
                    <Select
                      type={Select.TYPES.CHECKBOX}
                      label="Show Icon"
                      value={buttonShowIcon()}
                      onChange={setButtonShowIcon}
                    />
                    <Select
                      type={Select.TYPES.CHECKBOX}
                      label="Loading State"
                      value={buttonLoading()}
                      onChange={setButtonLoading}
                    />
                    <Select
                      type={Select.TYPES.CHECKBOX}
                      label="Disabled State"
                      value={buttonDisabled()}
                      onChange={setButtonDisabled}
                    />
                    <Select
                      type={Select.TYPES.CHECKBOX}
                      label="Full Width"
                      value={buttonFullWidth()}
                      onChange={setButtonFullWidth}
                    />
                  </>
                }
                preview={
                  <Button
                    size={buttonSize()}
                    variant={buttonVariant()}
                    loading={buttonLoading()}
                    disabled={buttonDisabled()}
                    fullWidth={buttonFullWidth()}
                    icon={buttonShowIcon() ? <EyeIcon /> : undefined}
                    iconPosition={buttonIconPosition()}
                  >
                    Test Button
                  </Button>
                }
                info={`Size: ${buttonSize()}
Variant: ${buttonVariant()}
Icon: ${buttonShowIcon() ? buttonIconPosition() : 'none'}${buttonLoading() ? '\nLoading: true' : ''}${buttonDisabled() ? '\nDisabled: true' : ''}${buttonFullWidth() ? '\nFull Width: true' : ''}`}
              />
            </TabPanel>

            {/* Input Tab */}
            <TabPanel id="input" activeTab={activeTab()}>
              <ComponentLayout
                title="Input"
                controls={
                  <>
                    <Select
                      type={Select.TYPES.SELECT}
                      label="Size"
                      size="sm"
                      placeholder="Select size"
                      options={Object.values(Input.SIZES).map((size) => ({
                        label: size.toUpperCase(),
                        value: size,
                      }))}
                      value={inputSize()}
                      onChange={setInputSize}
                    />
                    <Select
                      type={Select.TYPES.SELECT}
                      label="Variant"
                      size="sm"
                      placeholder="Select variant"
                      options={Object.values(Input.VARIANTS).map((variant) => ({
                        label: variant.charAt(0).toUpperCase() + variant.slice(1),
                        value: variant,
                      }))}
                      value={inputVariant()}
                      onChange={setInputVariant}
                    />
                    <Select
                      type={Select.TYPES.CHECKBOX}
                      label="Show Left Icon"
                      value={inputShowLeftIcon()}
                      onChange={setInputShowLeftIcon}
                    />
                    <Select
                      type={Select.TYPES.CHECKBOX}
                      label="Show Right Icon"
                      value={inputShowRightIcon()}
                      onChange={setInputShowRightIcon}
                    />
                    <Select
                      type={Select.TYPES.CHECKBOX}
                      label="Error State"
                      value={inputError()}
                      onChange={setInputError}
                    />
                    <Select
                      type={Select.TYPES.CHECKBOX}
                      label="Disabled State"
                      value={inputDisabled()}
                      onChange={setInputDisabled}
                    />
                    <Select
                      type={Select.TYPES.CHECKBOX}
                      label="Full Width"
                      value={inputFullWidth()}
                      onChange={setInputFullWidth}
                    />
                  </>
                }
                preview={
                  <Input
                    size={inputSize()}
                    variant={inputVariant()}
                    placeholder="Type something..."
                    value={inputValue()}
                    onInput={(e) => setInputValue(e.target.value)}
                    error={inputError() ? 'This field has an error' : undefined}
                    disabled={inputDisabled()}
                    fullWidth={inputFullWidth()}
                    leftIcon={inputShowLeftIcon() ? <SearchIcon /> : undefined}
                    rightIcon={inputShowRightIcon() ? <HeartIcon /> : undefined}
                  />
                }
                info={`Value: "${inputValue()}"
Size: ${inputSize()}
Variant: ${inputVariant()}
Icons: ${inputShowLeftIcon() ? 'Left ' : ''}${inputShowRightIcon() ? 'Right' : ''}${inputError() ? '\nError: true' : ''}${inputDisabled() ? '\nDisabled: true' : ''}`}
              />
            </TabPanel>

            {/* Select Tab */}
            <TabPanel id="select" activeTab={activeTab()}>
              <ComponentLayout
                title="Select"
                controls={
                  <>
                    <Select
                      type={Select.TYPES.SELECT}
                      label="Type"
                      size="sm"
                      placeholder="Select type"
                      options={[
                        { label: 'Dropdown Select', value: Select.TYPES.SELECT },
                        { label: 'Radio Group', value: Select.TYPES.RADIO },
                        { label: 'Multi Select', value: Select.TYPES.MULTI_SELECT },
                      ]}
                      value={selectType()}
                      onChange={setSelectType}
                    />
                    <Select
                      type={Select.TYPES.SELECT}
                      label="Size"
                      size="sm"
                      placeholder="Select size"
                      options={Object.values(Select.SIZES).map((size) => ({
                        label: size.toUpperCase(),
                        value: size,
                      }))}
                      value={selectSize()}
                      onChange={setSelectSize}
                    />
                    <Select
                      type={Select.TYPES.SELECT}
                      label="Variant"
                      size="sm"
                      placeholder="Select variant"
                      options={Object.values(Select.VARIANTS).map((variant) => ({
                        label: variant.charAt(0).toUpperCase() + variant.slice(1),
                        value: variant,
                      }))}
                      value={selectVariant()}
                      onChange={setSelectVariant}
                    />
                    <Select
                      type={Select.TYPES.SELECT}
                      label="Orientation"
                      size="sm"
                      placeholder="Select orientation"
                      options={Object.values(Select.ORIENTATIONS).map((orientation) => ({
                        label: orientation.charAt(0).toUpperCase() + orientation.slice(1),
                        value: orientation,
                      }))}
                      value={selectOrientation()}
                      onChange={setSelectOrientation}
                    />
                    <Select
                      type={Select.TYPES.CHECKBOX}
                      label="Error State"
                      value={selectError()}
                      onChange={setSelectError}
                    />
                    <Select
                      type={Select.TYPES.CHECKBOX}
                      label="Disabled State"
                      value={selectDisabled()}
                      onChange={setSelectDisabled}
                    />
                  </>
                }
                preview={
                  <Select
                    type={selectType()}
                    size={selectSize()}
                    variant={selectVariant()}
                    orientation={selectOrientation()}
                    placeholder="Choose an option"
                    label="Select Label"
                    description="This is a description for the select component"
                    options={testOptions}
                    value={selectValue()}
                    onChange={setSelectValue}
                    error={selectError()}
                    disabled={selectDisabled()}
                  />
                }
                info={`Type: ${selectType()}
Size: ${selectSize()}
Variant: ${selectVariant()}
Orientation: ${selectOrientation()}
Selected: "${selectValue()}"${selectError() ? '\nError: true' : ''}${selectDisabled() ? '\nDisabled: true' : ''}`}
              />
            </TabPanel>

            {/* List Tab */}
            <TabPanel id="list" activeTab={activeTab()}>
              <ComponentLayout
                title="List"
                controls={
                  <>
                    <Select
                      type={Select.TYPES.SELECT}
                      label="Type"
                      size="sm"
                      placeholder="Select type"
                      options={[
                        { label: 'Simple', value: List.TYPES.SIMPLE },
                        { label: 'Unordered', value: List.TYPES.UNORDERED },
                        { label: 'Ordered', value: List.TYPES.ORDERED },
                        { label: 'Description', value: List.TYPES.DESCRIPTION },
                      ]}
                      value={listType()}
                      onChange={setListType}
                    />
                    <Select
                      type={Select.TYPES.SELECT}
                      label="Size"
                      size="sm"
                      placeholder="Select size"
                      options={Object.values(List.SIZES).map((size) => ({
                        label: size.toUpperCase(),
                        value: size,
                      }))}
                      value={listSize()}
                      onChange={setListSize}
                    />
                    <Select
                      type={Select.TYPES.SELECT}
                      label="Variant"
                      size="sm"
                      placeholder="Select style"
                      options={Object.values(List.VARIANTS).map((variant) => ({
                        label: variant.charAt(0).toUpperCase() + variant.slice(1),
                        value: variant,
                      }))}
                      value={listVariant()}
                      onChange={setListVariant}
                    />
                    <Select
                      type={Select.TYPES.SELECT}
                      label="Orientation"
                      size="sm"
                      placeholder="Select orientation"
                      options={Object.values(List.ORIENTATIONS).map((orientation) => ({
                        label: orientation.charAt(0).toUpperCase() + orientation.slice(1),
                        value: orientation,
                      }))}
                      value={listOrientation()}
                      onChange={setListOrientation}
                    />
                    <Select
                      type={Select.TYPES.CHECKBOX}
                      label="Hoverable"
                      value={listHoverable()}
                      onChange={setListHoverable}
                    />
                    <Select
                      type={Select.TYPES.CHECKBOX}
                      label="Selectable"
                      value={listSelectable()}
                      onChange={setListSelectable}
                    />
                  </>
                }
                preview={
                  <div class="w-full max-w-sm">
                    <List
                      type={listType()}
                      size={listSize()}
                      variant={listVariant()}
                      orientation={listOrientation()}
                      hoverable={listHoverable()}
                      selectable={listSelectable()}
                      selectedValue={listSelectedValue()}
                      onItemSelect={setListSelectedValue}
                      items={
                        listType() === List.TYPES.DESCRIPTION
                          ? descriptionItems
                          : listType() === List.TYPES.SIMPLE &&
                              listVariant() === List.VARIANTS.DEFAULT
                            ? simpleListItems
                            : listItems
                      }
                    />
                    {listSelectedValue() && (
                      <div class="mt-3 p-2 bg-blue-50 dark:bg-blue-900/20 rounded text-sm">
                        Selected: {listSelectedValue()}
                      </div>
                    )}
                  </div>
                }
                info={`Type: ${listType()}
Size: ${listSize()}
Variant: ${listVariant()}
Orientation: ${listOrientation()}
Hoverable: ${listHoverable()}
Selectable: ${listSelectable()}
Selected: "${listSelectedValue()}"`}
              />
            </TabPanel>

            {/* Form Tab */}
            <TabPanel id="form" activeTab={activeTab()}>
              <ComponentLayout
                title="Form"
                controls={
                  <>
                    <Select
                      type={Select.TYPES.SELECT}
                      label="Size"
                      size="sm"
                      placeholder="Select size"
                      options={Object.values(Form.SIZES).map((size) => ({
                        label: size.toUpperCase(),
                        value: size,
                      }))}
                      value={formSize()}
                      onChange={setFormSize}
                    />
                    <Select
                      type={Select.TYPES.SELECT}
                      label="Variant"
                      size="sm"
                      placeholder="Select variant"
                      options={Object.values(Form.VARIANTS).map((variant) => ({
                        label: variant.charAt(0).toUpperCase() + variant.slice(1),
                        value: variant,
                      }))}
                      value={formVariant()}
                      onChange={setFormVariant}
                    />
                  </>
                }
                preview={
                  <div class="w-full max-w-lg">
                    <Form
                      size={formSize()}
                      variant={formVariant()}
                      initialValues={formInitialValues}
                      validationRules={formValidationRules}
                      onSubmit={async (values) => {
                        await new Promise((resolve) => setTimeout(resolve, 2000));
                        setFormSubmitMessage(values);
                      }}
                    >
                      <Form.Input
                        type="text"
                        name="name"
                        label="Full Name"
                        description="Enter your first and last name"
                        placeholder="e.g. John Doe"
                        rules={formValidationRules.name}
                      />

                      <Form.Input
                        type="email"
                        name="email"
                        label="Email Address"
                        description="We'll use this to contact you about your account"
                        placeholder="e.g. john@example.com"
                        rules={formValidationRules.email}
                      />

                      <Form.Input
                        type="password"
                        name="password"
                        label="Password"
                        description="Must be at least 6 characters long"
                        placeholder="Enter a secure password"
                        rules={formValidationRules.password}
                      />

                      <Form.Input
                        type="password"
                        name="confirmPassword"
                        label="Confirm Password"
                        description="Re-enter your password to confirm"
                        placeholder="Confirm your password"
                        rules={formValidationRules.confirmPassword}
                      />

                      <Form.Select
                        type={Select.TYPES.SELECT}
                        name="role"
                        label="Role"
                        description="Select your primary role or expertise"
                        placeholder="Choose your role"
                        options={roleOptions}
                        rules={formValidationRules.role}
                      />

                      <Form.Input
                        type="text"
                        name="bio"
                        label="Bio (Optional)"
                        description="Tell us a bit about yourself (max 200 characters)"
                        placeholder="I'm a developer who loves..."
                        rules={formValidationRules.bio}
                      />

                      <Form.Select
                        type={Select.TYPES.CHECKBOX}
                        name="agreeToTerms"
                        label="I agree to the terms and conditions"
                        description="Please read our terms before agreeing"
                        rules={formValidationRules.agreeToTerms}
                      />

                      <div class="flex gap-3 pt-4">
                        <Form.SubmitButton loadingText="Submitting...">
                          Submit Form
                        </Form.SubmitButton>
                        <Form.ResetButton>Reset</Form.ResetButton>
                      </div>

                      <Show when={formSubmitMessage()}>
                        <div class="mt-4 p-3 bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-md">
                          <h4 class="text-sm font-medium text-blue-800 dark:text-blue-200 mb-2">
                            Submitted Data:
                          </h4>
                          <pre class="text-xs text-blue-700 dark:text-blue-300 overflow-auto">
                            {JSON.stringify(formSubmitMessage(), null, 2)}
                          </pre>
                        </div>
                      </Show>
                    </Form>
                  </div>
                }
                info={`Size: ${formSize()}
Variant: ${formVariant()}
Status: ${formSubmitMessage() || 'Ready'}`}
              />
            </TabPanel>
          </Tabs>
        </Container>
      </PageContent>
    </Page>
  );
}
