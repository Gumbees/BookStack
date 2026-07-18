import { useNavigate, A } from '@solidjs/router';
import { Show, createSignal, createEffect, type ParentProps } from 'solid-js';
import { useAuth } from './auth';

function Header() {
  const auth = useAuth();
  const navigate = useNavigate();
  const [term, setTerm] = createSignal('');

  const submitSearch = (e: Event) => {
    e.preventDefault();
    const q = term().trim();
    if (q) navigate(`/search?query=${encodeURIComponent(q)}`);
  };

  return (
    <header class="app-header">
      <A href="/" class="brand">
        <span class="brand-mark">▤</span> BookStack
      </A>
      <form class="search-form" onSubmit={submitSearch}>
        <input
          type="search"
          placeholder="Search shelves, books, pages…"
          value={term()}
          onInput={e => setTerm(e.currentTarget.value)}
        />
      </form>
      <div class="header-right">
        <Show when={auth.user()}>
          {user => (
            <>
              <span class="user-chip" title={user().email}>
                {user().name}
                <span class={`role-badge role-${user().role}`}>{user().role}</span>
              </span>
              <button
                class="btn btn-ghost"
                onClick={() => {
                  auth.logout();
                  navigate('/login');
                }}
              >
                Sign out
              </button>
            </>
          )}
        </Show>
      </div>
    </header>
  );
}

export function Layout(props: ParentProps) {
  const auth = useAuth();
  const navigate = useNavigate();

  createEffect(() => {
    if (!auth.loggedIn()) navigate('/login', { replace: true });
  });

  return (
    <Show when={auth.loggedIn()}>
      <Header />
      <main class="app-main">{props.children}</main>
    </Show>
  );
}
