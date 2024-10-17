import { Router, Route } from '@solidjs/router';

import { MainLayout } from './layouts/MainLayout.jsx';
import { Debug } from './pages/Debug.jsx';
import { Home } from './pages/Home.jsx';

export function App() {
  return (
    <MainLayout>
      <Router>
        <Route path="/" component={Home} />
        <Route path="/debug" component={Debug} />
      </Router>
    </MainLayout>
  );
}
