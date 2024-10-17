import { createSignal, For } from 'solid-js';

import { DeviceStatusCard } from '../components/cards/DeviceStatusCard';
import { StatusCard } from '../components/cards/StatusCard';
import { Checkbox } from '../components/forms/Checkbox';
import { Form } from '../components/forms/Form';
import { FormField } from '../components/forms/FormField';
import { Input } from '../components/forms/Input';
import { Radio } from '../components/forms/Radio';
import { Select } from '../components/forms/Select';
import { Container } from '../components/layout/Container';
import { Page, PageHeader, PageContent, PageFooter } from '../components/layout/Page';
import { Button } from '../components/ui/Button';
import { useTheme } from '../context/ThemeContext';

export function Debug() {
  const theme = useTheme();
  const [showSuccess, setShowSuccess] = createSignal(false);
  const [selectedTab, setSelectedTab] = createSignal('buttons');

  // Mock device data
  const mockDevice = {
    id: 'device-1',
    name: 'Living Room Window',
    device_type: 'window',
    status: 'online',
    position: 75,
    battery: 85,
    temperature: 22.5,
    humidity: 45,
    light: 350,
    updated_at: new Date().toISOString(),
    previous: {
      position: 50,
      temperature: 21.0,
      humidity: 40,
      light: 300,
    },
  };

  // Mock sensor data
  const mockSensor = {
    id: 'sensor-1',
    name: 'Bedroom Sensor',
    device_type: 'sensor',
    status: 'online',
    battery: 90,
    temperature: 24.5,
    humidity: 55,
    light: 120,
    updated_at: new Date().toISOString(),
    previous: {
      temperature: 25.0,
      humidity: 52,
      light: 150,
    },
  };

  // Mock low battery device data
  const mockLowBatteryDevice = {
    id: 'device-2',
    name: 'Bathroom Window',
    device_type: 'window',
    status: 'warning',
    position: 30,
    battery: 15,
    temperature: 26.0,
    humidity: 65,
    light: 450,
    updated_at: new Date().toISOString(),
  };

  // Simple form validation
  const formValidator = {
    name: value => (!value ? 'Name is required' : undefined),
    email: value => {
      if (!value) return 'Email is required';
      if (!/\S+@\S+\.\S+/.test(value)) return 'Please enter a valid email address';
      return undefined;
    },
  };

  // Form initial values
  const initialValues = {
    name: '',
    email: '',
    role: 'user',
    notifications: true,
    preferences: '',
    agreeTerms: false,
    gender: 'undisclosed',
  };

  // Form submission handler
  const handleFormSubmit = async values => {
    console.log('Form values:', values);
    // Simulate API request
    await new Promise(resolve => setTimeout(resolve, 1000));
    setShowSuccess(true);
    setTimeout(() => setShowSuccess(false), 3000);
  };

  // Render button examples
  const renderButtons = () => {
    return (
      <div class="space-y-8">
        <div>
          <h2 class="text-lg font-medium mb-4">Button Variants</h2>
          <div class="flex flex-wrap gap-3">
            <Button variant={Button.VARIANTS.PRIMARY}>Primary Button</Button>
            <Button variant={Button.VARIANTS.SECONDARY}>Secondary Button</Button>
            <Button variant={Button.VARIANTS.GHOST}>Ghost Button</Button>
            <Button variant={Button.VARIANTS.DANGER}>Danger Button</Button>
          </div>
        </div>

        <div>
          <h2 class="text-lg font-medium mb-4">Button Sizes</h2>
          <div class="flex flex-wrap gap-3 items-center">
            <Button size={Button.SIZES.XS}>Extra Small</Button>
            <Button size={Button.SIZES.SM}>Small</Button>
            <Button size={Button.SIZES.MD}>Medium</Button>
            <Button size={Button.SIZES.LG}>Large</Button>
            <Button size={Button.SIZES.XL}>Extra Large</Button>
          </div>
        </div>

        <div>
          <h2 class="text-lg font-medium mb-4">Icon Buttons</h2>
          <div class="flex flex-wrap gap-3">
            <Button
              icon={
                <svg
                  xmlns="http://www.w3.org/2000/svg"
                  viewBox="0 0 20 20"
                  fill="currentColor"
                  class="w-5 h-5"
                >
                  <path d="M10 12a2 2 0 100-4 2 2 0 000 4z" />
                  <path
                    fill-rule="evenodd"
                    d="M.458 10C1.732 5.943 5.522 3 10 3s8.268 2.943 9.542 7c-1.274 4.057-5.064 7-9.542 7S1.732 14.057.458 10zM14 10a4 4 0 11-8 0 4 4 0 018 0z"
                    clip-rule="evenodd"
                  />
                </svg>
              }
            >
              With Icon
            </Button>
            <Button
              variant={Button.VARIANTS.SECONDARY}
              icon={
                <svg
                  xmlns="http://www.w3.org/2000/svg"
                  viewBox="0 0 20 20"
                  fill="currentColor"
                  class="w-5 h-5"
                >
                  <path
                    fill-rule="evenodd"
                    d="M16.704 4.153a.75.75 0 01.143 1.052l-8 10.5a.75.75 0 01-1.127.075l-4.5-4.5a.75.75 0 011.06-1.06l3.894 3.893 7.48-9.817a.75.75 0 011.05-.143z"
                    clip-rule="evenodd"
                  />
                </svg>
              }
              iconPosition="right"
            >
              Icon on Right
            </Button>
            <Button
              variant={Button.VARIANTS.GHOST}
              icon={
                <svg
                  xmlns="http://www.w3.org/2000/svg"
                  viewBox="0 0 20 20"
                  fill="currentColor"
                  class="w-5 h-5"
                >
                  <path
                    fill-rule="evenodd"
                    d="M10 18a8 8 0 100-16 8 8 0 000 16zM6.75 9.25a.75.75 0 000 1.5h6.5a.75.75 0 000-1.5h-6.5z"
                    clip-rule="evenodd"
                  />
                </svg>
              }
            >
              Ghost Button
            </Button>
          </div>
        </div>

        <div>
          <h2 class="text-lg font-medium mb-4">Button States</h2>
          <div class="flex flex-wrap gap-3">
            <Button>Normal Button</Button>
            <Button disabled>Disabled Button</Button>
            <Button fullWidth>Full Width Button</Button>
          </div>
        </div>
      </div>
    );
  };

  // Render form examples
  const renderForms = () => {
    return (
      <div class="space-y-8">
        <div>
          <h2 class="text-lg font-medium mb-4">Basic Form</h2>
          <Form
            initialValues={initialValues}
            validator={formValidator}
            onSubmit={handleFormSubmit}
            class="max-w-md p-6 bg-white dark:bg-gray-800 rounded-lg shadow-sm border border-gray-200 dark:border-gray-700"
          >
            <FormField name="name" label="Name" required hint="Please enter your full name">
              <Input placeholder="John Doe" />
            </FormField>

            <FormField name="email" label="Email" required hint="We won't share your email">
              <Input type="email" placeholder="example@example.com" />
            </FormField>

            <FormField name="role" label="Role" hint="Select your role">
              <Select
                options={[
                  { label: 'User', value: 'user' },
                  { label: 'Admin', value: 'admin' },
                  { label: 'Editor', value: 'editor' },
                ]}
              />
            </FormField>

            <FormField
              name="notifications"
              label="Receive Notifications"
              hint="Enable to receive updates"
            >
              <Checkbox />
            </FormField>

            <FormField name="agreeTerms" label="I agree to the Terms and Conditions" required>
              <Checkbox />
            </FormField>

            <FormField name="gender" label="Gender" hint="Select your gender">
              <Radio
                name="demo-radio"
                options={[
                  { label: 'Male', value: 'male' },
                  { label: 'Female', value: 'female' },
                  { label: 'Prefer not to say', value: 'undisclosed' },
                ]}
              />
            </FormField>

            <div class="mt-6 flex items-center justify-end gap-3">
              {showSuccess() && <p class="text-sm text-green-600">Submitted successfully!</p>}
              <Button type="submit" variant={Button.VARIANTS.PRIMARY}>
                Submit
              </Button>
            </div>
          </Form>
        </div>

        <div>
          <h2 class="text-lg font-medium mb-4">Input Component Showcase</h2>
          <div class="grid grid-cols-1 md:grid-cols-2 gap-4 max-w-4xl">
            <div class="p-4 bg-white dark:bg-gray-800 rounded-lg shadow-sm border border-gray-200 dark:border-gray-700">
              <h3 class="text-sm font-medium mb-2">Basic Input</h3>
              <Input placeholder="This is a basic input" />
            </div>

            <div class="p-4 bg-white dark:bg-gray-800 rounded-lg shadow-sm border border-gray-200 dark:border-gray-700">
              <h3 class="text-sm font-medium mb-2">Password Input</h3>
              <Input type="password" placeholder="Enter password" />
            </div>

            <div class="p-4 bg-white dark:bg-gray-800 rounded-lg shadow-sm border border-gray-200 dark:border-gray-700">
              <h3 class="text-sm font-medium mb-2">Number Input</h3>
              <Input type="number" placeholder="Enter a number" />
            </div>

            <div class="p-4 bg-white dark:bg-gray-800 rounded-lg shadow-sm border border-gray-200 dark:border-gray-700">
              <h3 class="text-sm font-medium mb-2">Disabled Input</h3>
              <Input placeholder="Disabled state" disabled />
            </div>

            <div class="p-4 bg-white dark:bg-gray-800 rounded-lg shadow-sm border border-gray-200 dark:border-gray-700">
              <h3 class="text-sm font-medium mb-2">Input with Left Icon</h3>
              <Input
                placeholder="Search..."
                leftIcon={
                  <svg
                    xmlns="http://www.w3.org/2000/svg"
                    viewBox="0 0 20 20"
                    fill="currentColor"
                    class="w-5 h-5 text-gray-400"
                  >
                    <path
                      fill-rule="evenodd"
                      d="M9 3.5a5.5 5.5 0 100 11 5.5 5.5 0 000-11zM2 9a7 7 0 1112.452 4.391l3.328 3.329a.75.75 0 11-1.06 1.06l-3.329-3.328A7 7 0 012 9z"
                      clip-rule="evenodd"
                    />
                  </svg>
                }
              />
            </div>

            <div class="p-4 bg-white dark:bg-gray-800 rounded-lg shadow-sm border border-gray-200 dark:border-gray-700">
              <h3 class="text-sm font-medium mb-2">Input with Right Icon</h3>
              <Input
                placeholder="Menu..."
                rightIcon={
                  <svg
                    xmlns="http://www.w3.org/2000/svg"
                    viewBox="0 0 20 20"
                    fill="currentColor"
                    class="w-5 h-5 text-gray-400"
                  >
                    <path
                      fill-rule="evenodd"
                      d="M2.25 12c0-1.1.9-2 2-2h13.5a.75.75 0 010 1.5H4.25c-.25 0-.5.25-.5.5v2.25c0 .25.25.5.5.5h13.5a.75.75 0 010 1.5H4.25A2.25 2.25 0 012 14.25V12z"
                      clip-rule="evenodd"
                    />
                  </svg>
                }
              />
            </div>

            <div class="p-4 bg-white dark:bg-gray-800 rounded-lg shadow-sm border border-gray-200 dark:border-gray-700">
              <h3 class="text-sm font-medium mb-2">Checkbox</h3>
              <Checkbox label="Check me" />
            </div>

            <div class="p-4 bg-white dark:bg-gray-800 rounded-lg shadow-sm border border-gray-200 dark:border-gray-700">
              <h3 class="text-sm font-medium mb-2">Radio Buttons</h3>
              <Radio
                name="demo-radio"
                options={[
                  { label: 'Option 1', value: '1' },
                  { label: 'Option 2', value: '2' },
                  { label: 'Disabled Option', value: '3', disabled: true },
                ]}
              />
            </div>
          </div>
        </div>
      </div>
    );
  };

  // Render card examples
  const renderCards = () => {
    return (
      <div class="space-y-8">
        <div>
          <h2 class="text-lg font-medium mb-4">Status Cards</h2>
          <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            <StatusCard
              title="Default Status"
              value="42"
              unit="units"
              description="This is a default card description"
              variant={StatusCard.VARIANTS.DEFAULT}
            />
            <StatusCard
              title="Success Status"
              value="85"
              unit="%"
              description="Operation completed"
              variant={StatusCard.VARIANTS.SUCCESS}
              trend="up"
              trendValue="5%"
            />
            <StatusCard
              title="Warning Status"
              value="65"
              unit="°C"
              description="Approaching threshold"
              variant={StatusCard.VARIANTS.WARNING}
              trend="up"
              trendValue="2.5°"
            />
            <StatusCard
              title="Error Status"
              value="15"
              unit="MB/s"
              description="Connection speed too low"
              variant={StatusCard.VARIANTS.ERROR}
              trend="down"
              trendValue="5MB/s"
            />
            <StatusCard
              title="Info Status"
              value="1.2K"
              unit="lux"
              description="Current light level"
              variant={StatusCard.VARIANTS.INFO}
              trend="neutral"
            />
            <StatusCard
              title="Loading State"
              loading={true}
              variant={StatusCard.VARIANTS.DEFAULT}
            />
          </div>
        </div>

        <div>
          <h2 class="text-lg font-medium mb-4">Device Status Cards</h2>
          <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            <DeviceStatusCard
              device={mockDevice}
              onClick={() => console.log('Clicked device card', mockDevice.id)}
            />
            <DeviceStatusCard
              device={mockSensor}
              onClick={() => console.log('Clicked sensor card', mockSensor.id)}
            />
            <DeviceStatusCard
              device={mockLowBatteryDevice}
              onClick={() =>
                console.log('Clicked low battery device card', mockLowBatteryDevice.id)
              }
            />
            <DeviceStatusCard loading={true} />
          </div>
        </div>
      </div>
    );
  };

  // Render layout examples
  const renderLayouts = () => {
    return (
      <div class="space-y-8">
        <div>
          <h2 class="text-lg font-medium mb-4">Containers</h2>
          <div class="space-y-4">
            <For each={Object.values(Container.SIZES)}>
              {size => (
                <div class="border border-dashed border-gray-300 dark:border-gray-700 rounded-md overflow-hidden">
                  <Container size={size} class="bg-gray-100 dark:bg-gray-800 p-4 text-center">
                    <p>Container size: {size}</p>
                  </Container>
                </div>
              )}
            </For>
          </div>
        </div>
      </div>
    );
  };

  // Switch tabs
  const renderContent = () => {
    switch (selectedTab()) {
      case 'buttons':
        return renderButtons();
      case 'forms':
        return renderForms();
      case 'cards':
        return renderCards();
      case 'layouts':
        return renderLayouts();
      default:
        return renderButtons();
    }
  };

  return (
    <Page title="Component Debug Page">
      <PageHeader
        title="Component Library Debug"
        description="Test and debug all UI components"
        border
      >
        <div class="flex mt-4 space-x-1 overflow-x-auto pb-2">
          <For
            each={[
              { id: 'buttons', label: 'Buttons' },
              { id: 'forms', label: 'Forms' },
              { id: 'cards', label: 'Cards' },
              { id: 'layouts', label: 'Layouts' },
            ]}
          >
            {tab => (
              <Button
                variant={
                  selectedTab() === tab.id ? Button.VARIANTS.PRIMARY : Button.VARIANTS.GHOST
                }
                size={Button.SIZES.SM}
                onClick={() => setSelectedTab(tab.id)}
              >
                {tab.label}
              </Button>
            )}
          </For>
        </div>
      </PageHeader>

      <PageContent>
        <Container>{renderContent()}</Container>
      </PageContent>

      <PageFooter border>
        <div class="text-center text-sm text-gray-500">
          Theme: {theme.theme()} |
          <Button
            variant={Button.VARIANTS.GHOST}
            size={Button.SIZES.XS}
            onClick={() => {
              const currentTheme = theme.theme();
              const newTheme =
                currentTheme === theme.themes.DARK ? theme.themes.LIGHT : theme.themes.DARK;
              theme.setTheme(newTheme);
            }}
            class="ml-2"
          >
            Toggle Theme
          </Button>
        </div>
      </PageFooter>
    </Page>
  );
}
