import { createResource, createSignal, For, Show } from 'solid-js';
import { useSearchParams } from '@solidjs/router';
import { api } from '../api';
import { useAuth } from '../auth';

/// OAuth consent: `GET /oauth/authorize` validates the request server-side
/// and redirects here. The user (already signed in via password or SSO —
/// the Layout guard sends them to /login first otherwise) picks the org the
/// token will act in and approves; we then bounce back to the client's
/// redirect_uri with the authorization code.
export default function OauthConsent() {
  const [params] = useSearchParams();
  const auth = useAuth();
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [orgId, setOrgId] = createSignal<number | null>(null);

  const str = (v: unknown) => (typeof v === 'string' ? v : '');

  const [client] = createResource(
    () => str(params.client_id),
    id => (id ? api.get<{ name: string; client_id: string }>(`/oauth/client/${id}`) : Promise.resolve(null)),
  );

  const approve = async () => {
    setBusy(true);
    setError(null);
    try {
      const res = await api.post<{ redirect_to: string }>('/oauth/approve', {
        client_id: str(params.client_id),
        redirect_uri: str(params.redirect_uri),
        state: str(params.state),
        code_challenge: str(params.code_challenge),
        code_challenge_method: str(params.code_challenge_method) || 'S256',
        scope: str(params.scope) || 'bookstack',
        org_id: orgId() ?? auth.activeOrg()?.org_id,
      });
      location.assign(res.redirect_to);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'authorization failed');
      setBusy(false);
    }
  };

  return (
    <div class="consent-wrap">
      <div class="consent-card">
        <h1>Authorize access</h1>
        <p>
          <strong>{client()?.name ?? str(params.client_id)}</strong> wants to access BookStack as{' '}
          <strong>{auth.user()?.name}</strong>.
        </p>
        <label class="consent-org">
          Organization this connection may act in:
          <select
            value={String(orgId() ?? auth.activeOrg()?.org_id ?? '')}
            onChange={e => setOrgId(Number(e.currentTarget.value))}
          >
            <For each={auth.orgs()}>
              {membership => <option value={String(membership.org_id)}>{membership.name}</option>}
            </For>
          </select>
        </label>
        <p class="muted-note">
          The connection gets your role in that org ({auth.activeOrg()?.role}) — it can read
          everything you can, and edit if you can.
        </p>
        <Show when={error()}>{message => <div class="form-error">{message()}</div>}</Show>
        <div class="btn-row">
          <button class="btn btn-primary" disabled={busy()} onClick={approve}>
            {busy() ? 'Authorizing…' : 'Authorize'}
          </button>
          <button class="btn" onClick={() => history.back()}>
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
}
