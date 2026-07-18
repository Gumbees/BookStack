import { createResource, createSignal, For, onMount, Show } from 'solid-js';
import { useNavigate } from '@solidjs/router';
import { useAuth } from '../auth';
import { api } from '../api';
import type { PublicProvider } from '../types';

export default function Login() {
  const auth = useAuth();
  const navigate = useNavigate();
  const [email, setEmail] = createSignal('');
  const [password, setPassword] = createSignal('');
  const [error, setError] = createSignal<string | null>(null);
  const [busy, setBusy] = createSignal(false);
  const [providers] = createResource(() =>
    api.get<{ data: PublicProvider[] }>('/auth/providers').catch(() => ({ data: [] })),
  );

  // The SSO callback lands here with #sso_token=… (or #sso_error=…).
  onMount(async () => {
    const hash = new URLSearchParams(location.hash.replace(/^#/, ''));
    const ssoToken = hash.get('sso_token');
    const ssoError = hash.get('sso_error');
    if (ssoError) {
      setError(decodeURIComponent(ssoError));
      history.replaceState(null, '', location.pathname);
    }
    if (ssoToken) {
      history.replaceState(null, '', location.pathname);
      try {
        await auth.adoptSsoToken(ssoToken);
        navigate('/', { replace: true });
      } catch {
        setError('single sign-on failed');
      }
    }
  });

  const submit = async (e: Event) => {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      await auth.login(email(), password());
      navigate('/', { replace: true });
    } catch (err) {
      setError(err instanceof Error ? err.message : 'login failed');
    } finally {
      setBusy(false);
    }
  };

  return (
    <div class="login-wrap">
      <form class="login-card" onSubmit={submit}>
        <h1>
          <span class="brand-mark">▤</span> BookStack
        </h1>
        <p class="login-sub">Sign in to your knowledge base</p>
        <label>
          Email
          <input
            type="email"
            value={email()}
            onInput={e => setEmail(e.currentTarget.value)}
            autocomplete="username"
            required
          />
        </label>
        <label>
          Password
          <input
            type="password"
            value={password()}
            onInput={e => setPassword(e.currentTarget.value)}
            autocomplete="current-password"
            required
          />
        </label>
        <Show when={error()}>{message => <div class="form-error">{message()}</div>}</Show>
        <button class="btn btn-primary" type="submit" disabled={busy()}>
          {busy() ? 'Signing in…' : 'Sign in'}
        </button>
        <Show when={(providers()?.data ?? []).length > 0}>
          <div class="sso-divider">or continue with</div>
          <div class="sso-buttons">
            <For each={providers()?.data}>
              {provider => (
                <a class="btn sso-btn" href={`/api/auth/oidc/${provider.id}/start?redirect=/login`}>
                  {provider.name}
                </a>
              )}
            </For>
          </div>
        </Show>
      </form>
    </div>
  );
}
