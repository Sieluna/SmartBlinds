import { createSignal, onMount, For, Show, createMemo } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';
import { Button } from '../components/ui/Button.jsx';
import { Form } from '../components/ui/Form.jsx';
import { useTranslation } from '../context/LanguageContext.jsx';

export function Stepper() {
  const { t } = useTranslation();
  
  // State management
  const [endpoint, setEndpoint] = createSignal('192.168.1.100:8082');
  const [isConnected, setIsConnected] = createSignal(false);
  const [currentStatus, setCurrentStatus] = createSignal(null);
  const [testResults, setTestResults] = createSignal([]);
  const [isRunning, setIsRunning] = createSignal(false);

  // Form state for manual commands
  const formStore = Form.createFormStore({
    moveSteps: 100,
    speed: 300,
    acceleration: 150,
    customEndpoint: '192.168.1.100:8082',
  });

  // Test connection with ping
  const testConnection = async () => {
    setIsRunning(true);
    try {
      const result = await invoke('test_stepper', {
        action: {
          Single: {
            endpoint: endpoint(),
            command: 'Ping'
          }
        }
      });
      
      if (result.Single?.success) {
        setIsConnected(true);
        addResult('Connection Test', result.Single, true);
      } else {
        setIsConnected(false);
        addResult('Connection Test', result.Single, false);
      }
    } catch (error) {
      setIsConnected(false);
      addResult('Connection Test', { error: error.toString() }, false);
    }
    setIsRunning(false);
  };

  // Get current status
  const getStatus = async () => {
    setIsRunning(true);
    try {
      const result = await invoke('test_stepper', {
        action: {
          Single: {
            endpoint: endpoint(),
            command: 'Status'
          }
        }
      });
      
      if (result.Single?.success && result.Single.response?.Status) {
        setCurrentStatus(result.Single.response.Status);
        addResult('Status Query', result.Single, true);
      } else {
        addResult('Status Query', result.Single, false);
      }
    } catch (error) {
      addResult('Status Query', { error: error.toString() }, false);
    }
    setIsRunning(false);
  };

  // Execute single command
  const executeCommand = async (command) => {
    setIsRunning(true);
    try {
      const result = await invoke('test_stepper', {
        action: {
          Single: {
            endpoint: endpoint(),
            command
          }
        }
      });
      
      const commandName = typeof command === 'string' ? command : JSON.stringify(command);
      addResult(commandName, result.Single, result.Single?.success || false);
      
      // Update status if this was a status command
      if (result.Single?.response?.Status) {
        setCurrentStatus(result.Single.response.Status);
      }
    } catch (error) {
      const commandName = typeof command === 'string' ? command : JSON.stringify(command);
      addResult(commandName, { error: error.toString() }, false);
    }
    setIsRunning(false);
  };

  // Run predefined test sequence
  const runTestSequence = async () => {
    setIsRunning(true);
    try {
      const result = await invoke('test_stepper', {
        action: {
          Sequence: {
            endpoint: endpoint()
          }
        }
      });
      
      if (result.Sequence) {
        addResult(
          `Test Sequence (${result.Sequence.successful_tests}/${result.Sequence.total_tests})`,
          result.Sequence,
          result.Sequence.successful_tests === result.Sequence.total_tests
        );
        
        // Add individual results
        result.Sequence.results.forEach((testResult, index) => {
          const commandName = typeof testResult.command === 'string' 
            ? testResult.command 
            : JSON.stringify(testResult.command);
          addResult(`Seq #${index + 1}: ${commandName}`, testResult, testResult.success);
        });
      }
    } catch (error) {
      addResult('Test Sequence', { error: error.toString() }, false);
    }
    setIsRunning(false);
  };

  // Run custom movement test
  const runMovementTest = async (values) => {
    setIsRunning(true);
    const commands = [
      'Ping',
      'Status',
      'Home',
      { SetSpeed: values.speed },
      { SetAcceleration: values.acceleration },
      { Move: values.moveSteps },
      'Status',
      'Home',
      'Status'
    ];

    try {
      const result = await invoke('test_stepper', {
        action: {
          Custom: {
            endpoint: values.customEndpoint,
            commands
          }
        }
      });
      
      if (result.Custom) {
        addResult(
          `Movement Test (${result.Custom.successful_tests}/${result.Custom.total_tests})`,
          result.Custom,
          result.Custom.successful_tests === result.Custom.total_tests
        );
      }
    } catch (error) {
      addResult('Movement Test', { error: error.toString() }, false);
    }
    setIsRunning(false);
  };

  // Add result to the list
  const addResult = (name, result, success) => {
    const timestamp = new Date().toLocaleTimeString();
    setTestResults(prev => [{
      id: Date.now(),
      name,
      result,
      success,
      timestamp
    }, ...prev.slice(0, 49)]); // Keep last 50 results
  };

  // Clear results
  const clearResults = () => {
    setTestResults([]);
    setCurrentStatus(null);
  };

  // Format result for display
  const formatResult = (result) => {
    if (result.error) {
      return result.error;
    }
    if (result.response) {
      return JSON.stringify(result.response, null, 2);
    }
    if (result.total_tests !== undefined) {
      return `Completed in ${result.total_duration_ms}ms`;
    }
    return JSON.stringify(result, null, 2);
  };

  const StatusDisplay = () => (
    <Show when={currentStatus()}>
      <div class="bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg p-4">
        <h3 class="font-semibold text-blue-900 dark:text-blue-100 mb-2">Current Status</h3>
        <div class="grid grid-cols-2 gap-4 text-sm">
          <div>
            <span class="text-gray-600 dark:text-gray-400">Position:</span>
            <span class="ml-2 font-mono">{currentStatus().position}</span>
          </div>
          <div>
            <span class="text-gray-600 dark:text-gray-400">Target:</span>
            <span class="ml-2 font-mono">{currentStatus().target}</span>
          </div>
          <div>
            <span class="text-gray-600 dark:text-gray-400">Speed:</span>
            <span class="ml-2 font-mono">{currentStatus().speed}</span>
          </div>
          <div>
            <span class="text-gray-600 dark:text-gray-400">Running:</span>
            <span class={`ml-2 font-semibold ${currentStatus().running ? 'text-green-600' : 'text-gray-600'}`}>
              {currentStatus().running ? 'Yes' : 'No'}
            </span>
          </div>
        </div>
      </div>
    </Show>
  );

  const ConnectionStatus = () => (
    <div class={`flex items-center gap-2 px-3 py-2 rounded text-sm font-medium ${
      isConnected() 
        ? 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-300'
        : 'bg-gray-100 text-gray-800 dark:bg-gray-900/30 dark:text-gray-300'
    }`}>
      <div class={`w-2 h-2 rounded-full ${
        isConnected() ? 'bg-green-500 animate-pulse' : 'bg-gray-400'
      }`} />
      {isConnected() ? 'Connected' : 'Disconnected'}
    </div>
  );

  return (
    <div class="min-h-screen bg-gray-50 dark:bg-gray-900 pb-20">
      <div class="p-6">
        <h1 class="text-2xl font-bold mb-2">Stepper Motor Tester</h1>
        <p class="text-gray-600 dark:text-gray-400 mb-6">
          Test and control stepper motor devices over TCP connection
        </p>

        {/* Connection Section */}
        <div class="bg-white dark:bg-gray-800 rounded-lg border p-6 mb-6">
          <div class="flex items-center justify-between mb-4">
            <h2 class="text-lg font-semibold">Connection</h2>
            <ConnectionStatus />
          </div>
          
          <div class="flex gap-4 items-end">
            <div class="flex-1">
              <label class="block text-sm font-medium mb-1">Device Endpoint</label>
              <input
                type="text"
                value={endpoint()}
                onInput={(e) => setEndpoint(e.target.value)}
                placeholder="192.168.1.100:8082"
                class="w-full px-3 py-2 border rounded-md bg-white dark:bg-gray-700 border-gray-300 dark:border-gray-600"
              />
            </div>
            <Button 
              onClick={testConnection} 
              loading={isRunning()}
              variant="primary"
            >
              Test Connection
            </Button>
            <Button 
              onClick={getStatus} 
              loading={isRunning()}
              variant="secondary"
              disabled={!isConnected()}
            >
              Get Status
            </Button>
          </div>
        </div>

        {/* Status Display */}
        <StatusDisplay />

        <div class="grid lg:grid-cols-2 gap-6 mt-6">
          {/* Control Panel */}
          <div class="bg-white dark:bg-gray-800 rounded-lg border p-6">
            <h2 class="text-lg font-semibold mb-4">Manual Control</h2>
            
            <div class="space-y-4">
              {/* Quick Commands */}
              <div>
                <h3 class="font-medium mb-2">Quick Commands</h3>
                <div class="grid grid-cols-2 gap-2">
                  <Button 
                    onClick={() => executeCommand('Home')} 
                    loading={isRunning()}
                    disabled={!isConnected()}
                    size="sm"
                  >
                    Home
                  </Button>
                  <Button 
                    onClick={() => executeCommand('Stop')} 
                    loading={isRunning()}
                    disabled={!isConnected()}
                    variant="destructive"
                    size="sm"
                  >
                    Stop
                  </Button>
                </div>
              </div>

              {/* Movement Controls */}
              <Form formStore={formStore} onSubmit={runMovementTest}>
                <div class="space-y-3">
                  <Form.Input
                    name="moveSteps"
                    type="number"
                    label="Steps to Move"
                    min="-10000"
                    max="10000"
                  />
                  <Form.Input
                    name="speed"
                    type="number"
                    label="Speed"
                    min="1"
                    max="2000"
                    step="1"
                  />
                  <Form.Input
                    name="acceleration"
                    type="number"
                    label="Acceleration"
                    min="1"
                    max="1000"
                    step="1"
                  />
                  <Form.Input
                    name="customEndpoint"
                    label="Endpoint (for test)"
                    placeholder="192.168.1.100:8082"
                  />
                </div>
                
                <div class="flex gap-2 mt-4">
                  <Form.SubmitButton 
                    loading={isRunning()}
                    disabled={!isConnected()}
                    size="sm"
                  >
                    Run Movement Test
                  </Form.SubmitButton>
                  <Button 
                    onClick={runTestSequence} 
                    loading={isRunning()}
                    disabled={!isConnected()}
                    variant="secondary"
                    size="sm"
                  >
                    Full Test Sequence
                  </Button>
                </div>
              </Form>
            </div>
          </div>

          {/* Results Panel */}
          <div class="bg-white dark:bg-gray-800 rounded-lg border p-6">
            <div class="flex items-center justify-between mb-4">
              <h2 class="text-lg font-semibold">Test Results</h2>
              <Button onClick={clearResults} variant="ghost" size="sm">
                Clear
              </Button>
            </div>

            <div class="space-y-2 max-h-96 overflow-y-auto">
              <Show
                when={testResults().length > 0}
                fallback={
                  <div class="text-center py-8 text-gray-500">
                    No test results yet. Run a command to see results here.
                  </div>
                }
              >
                <For each={testResults()}>
                  {(result) => (
                    <div class={`p-3 rounded border-l-4 ${
                      result.success 
                        ? 'border-green-500 bg-green-50 dark:bg-green-900/10' 
                        : 'border-red-500 bg-red-50 dark:bg-red-900/10'
                    }`}>
                      <div class="flex items-center justify-between mb-1">
                        <span class="font-medium text-sm">{result.name}</span>
                        <div class="flex items-center gap-2">
                          <span class="text-xs text-gray-500">{result.timestamp}</span>
                          <span class={`text-xs font-medium ${
                            result.success ? 'text-green-600' : 'text-red-600'
                          }`}>
                            {result.success ? '✓' : '✗'}
                          </span>
                        </div>
                      </div>
                      <pre class="text-xs text-gray-600 dark:text-gray-400 whitespace-pre-wrap overflow-hidden">
                        {formatResult(result.result)}
                      </pre>
                    </div>
                  )}
                </For>
              </Show>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
} 