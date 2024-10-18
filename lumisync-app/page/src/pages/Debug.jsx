import { createSignal } from 'solid-js';
import { Button } from '../components/ui/Button.jsx';
import { Input } from '../components/ui/Input.jsx';
import { Select } from '../components/ui/Select.jsx';
import { Tabs, TabPanel } from '../components/layout/Tabs.jsx';
import { Page, PageContent } from '../components/layout/Page.jsx';
import { Container } from '../components/layout/Container.jsx';

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

  // Tab definitions
  const tabs = [
    { id: 'button', label: 'Button' },
    { id: 'input', label: 'Input' },
    { id: 'select', label: 'Select' },
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
          </Tabs>
        </Container>
      </PageContent>
    </Page>
  );
}
