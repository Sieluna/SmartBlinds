import { Router, Route } from '@solidjs/router';

import { MainLayout } from './layouts/MainLayout.jsx';
import { Debug } from './pages/Debug.jsx';
import { Home } from './pages/Home.jsx';
import { Networks } from './pages/Networks.jsx';
import { Devices } from './pages/Devices.jsx';
import { Stepper } from './pages/Stepper.jsx';

export function App() {
  return (
    <MainLayout>
      <Router>
        <Route path="/" component={Home} />
        <Route path="/debug" component={Debug} />
        <Route path="/networks" component={Networks} />
        <Route path="/devices" component={Devices} />
        <Route path="/stepper" component={Stepper} />
      </Router>
    </MainLayout>
  );
}
