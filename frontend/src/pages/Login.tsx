import { createSignal, Show } from 'solid-js';
import { useNavigate } from '@solidjs/router';
import { useAuth } from '../auth';

export default function Login() {
  const auth = useAuth();
  const navigate = useNavigate();
  const [email, setEmail] = createSignal('');
  const [password, setPassword] = createSignal('');
  const [error, setError] = createSignal<string | null>(null);
  const [busy, setBusy] = createSignal(false);

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
      </form>
    </div>
  );
}
