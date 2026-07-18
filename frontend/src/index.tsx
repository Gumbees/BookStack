/* @refresh reload */
import { render } from 'solid-js/web';
import { Route, Router } from '@solidjs/router';
import { Layout } from './App';
import { AuthProvider } from './auth';
import Login from './pages/Login';
import Dashboard from './pages/Dashboard';
import ShelfView from './pages/ShelfView';
import BookView from './pages/BookView';
import PageView from './pages/PageView';
import PageEditor from './pages/PageEditor';
import SearchResults from './pages/SearchResults';
import './styles.css';

render(
  () => (
    <AuthProvider>
      <Router>
        <Route path="/login" component={Login} />
        <Route path="/" component={() => <Layout><Dashboard /></Layout>} />
        <Route path="/shelf/:slug" component={() => <Layout><ShelfView /></Layout>} />
        <Route path="/book/:slug" component={() => <Layout><BookView /></Layout>} />
        <Route path="/book/:bookSlug/page/:pageSlug" component={() => <Layout><PageView /></Layout>} />
        <Route path="/book/:bookSlug/page/:pageSlug/edit" component={() => <Layout><PageEditor /></Layout>} />
        <Route path="/search" component={() => <Layout><SearchResults /></Layout>} />
        <Route path="*" component={() => <Layout><div class="empty-note">Not found.</div></Layout>} />
      </Router>
    </AuthProvider>
  ),
  document.getElementById('root')!,
);
