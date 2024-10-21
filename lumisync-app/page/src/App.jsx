import { Router, Route } from '@solidjs/router';

import { MainLayout } from './layouts/MainLayout.jsx';
import { Debug } from './pages/Debug.jsx';
import { Home } from './pages/Home.jsx';
import { Wifi } from './pages/Wifi.jsx';

export function App() {
  return (
    <MainLayout>
      <Router>
        <Route path="/" component={Home} />
        <Route path="/debug" component={Debug} />
        <Route path="/wifi" component={Wifi} />
      </Router>
    </MainLayout>
  );
}
