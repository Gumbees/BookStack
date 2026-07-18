import { useNavigate, A } from '@solidjs/router';
import { For, Show, createSignal, createEffect, type ParentProps } from 'solid-js';
import { useAuth } from './auth';
import { branding, loadBranding, logoUrl } from './branding';

function Header() {
  const auth = useAuth();
  const navigate = useNavigate();
  const [term, setTerm] = createSignal('');
  const [mode, setMode] = createSignal<'keyword' | 'semantic' | 'precision'>('keyword');
  const [allOrgs, setAllOrgs] = createSignal(false);

  const submitSearch = (e: Event) => {
    e.preventDefault();
    const q = term().trim();
    if (!q) return;
    const params = new URLSearchParams({ query: q, mode: mode() });
    if (allOrgs()) params.set('scope', 'global');
    navigate(`/search?${params.toString()}`);
  };

  return (
    <header class="app-header">
      <A href="/" class="brand">
        <Show
          when={branding().has_logo}
          fallback={<span class="brand-mark">▤</span>}
        >
          <img class="brand-logo" src={logoUrl(auth.activeOrg()?.org_id ?? null)} alt="" />
        </Show>
        {branding().name}
      </A>
      <form class="search-form" onSubmit={submitSearch}>
        <input
          type="search"
          placeholder="Search shelves, books, pages…"
          value={term()}
          onInput={e => setTerm(e.currentTarget.value)}
        />
        <select
          class="search-mode"
          value={mode()}
          onChange={e => setMode(e.currentTarget.value as 'keyword' | 'semantic' | 'precision')}
          title="Search mode"
        >
          <option value="keyword">Keyword</option>
          <option value="semantic">Semantic</option>
          <option value="precision">Precision</option>
        </select>
        <label class="search-global" title="Search every org you belong to">
          <input type="checkbox" checked={allOrgs()} onChange={e => setAllOrgs(e.currentTarget.checked)} />
          all orgs
        </label>
      </form>
      <div class="header-right">
        <Show when={auth.activeOrg()}>
          {org => (
            <div class="org-switcher" title="Active organization">
              <span class="org-icon">🏢</span>
              <select
                value={String(org().org_id)}
                onChange={e => auth.switchOrg(Number(e.currentTarget.value))}
              >
                <For each={auth.orgs()}>
                  {membership => (
                    <option value={String(membership.org_id)}>{membership.name}</option>
                  )}
                </For>
              </select>
            </div>
          )}
        </Show>
        <Show when={auth.isOrgAdmin()}>
          <A href="/admin" class="btn btn-ghost" title="Admin settings">
            ⚙
          </A>
        </Show>
        <Show when={auth.user()}>
          {user => (
            <>
              <span class="user-chip" title={user().email}>
                {user().name}
                <span class={`role-badge role-${auth.activeOrg()?.role ?? user().role}`}>
                  {auth.activeOrg()?.role ?? user().role}
                </span>
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

  // Theme the page for the active org (colors + logo + display name).
  createEffect(() => {
    loadBranding(auth.activeOrg()?.org_id ?? null);
  });

  return (
    <Show when={auth.loggedIn()}>
      <Header />
      <main class="app-main">{props.children}</main>
    </Show>
  );
}
