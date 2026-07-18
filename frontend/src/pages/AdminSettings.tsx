import { createEffect, createResource, createSignal, For, onCleanup, Show } from 'solid-js';
import { api, getToken } from '../api';
import { loadBranding } from '../branding';
import { useAuth } from '../auth';
import type { AuthProvider, OrgMemberInfo } from '../types';

/// Admin settings: an org tab (members + org auth providers, gated by the
/// global checkbox) and a global tab for system admins (the checkbox itself
/// plus instance-wide auth providers, inherited by every org by default).
export default function AdminSettings() {
  const auth = useAuth();
  const [tab, setTab] = createSignal<'org' | 'global' | 'data'>('org');

  return (
    <div>
      <h1>⚙ Admin settings</h1>
      <div class="tab-row">
        <button class={`tab ${tab() === 'org' ? 'tab-active' : ''}`} onClick={() => setTab('org')}>
          {auth.activeOrg()?.name ?? 'This org'}
        </button>
        <Show when={auth.isSystemAdmin()}>
          <button
            class={`tab ${tab() === 'global' ? 'tab-active' : ''}`}
            onClick={() => setTab('global')}
          >
            Global
          </button>
        </Show>
        <button class={`tab ${tab() === 'data' ? 'tab-active' : ''}`} onClick={() => setTab('data')}>
          Data
        </button>
      </div>
      <Show when={tab() === 'org'}>
        <OrgTab />
      </Show>
      <Show when={tab() === 'global'}>
        <GlobalTab />
      </Show>
      <Show when={tab() === 'data'}>
        <DataTab />
      </Show>
    </div>
  );
}

interface BackupRow {
  id: number;
  scope: string;
  org_id: number | null;
  target: string;
  location: string;
  size_bytes: number;
  status: string;
  error: string | null;
  created_at: string;
}

interface ImportRow {
  id: number;
  org_id: number;
  source_url: string;
  status: string;
  progress: Record<string, unknown>;
}

function DataTab() {
  const auth = useAuth();
  const orgId = () => auth.activeOrg()?.org_id ?? 0;

  const [imports, { refetch: refetchImports }] = createResource(() =>
    api.get<{ data: ImportRow[] }>('/admin/imports'),
  );
  const [backups, { refetch: refetchBackups }] = createResource(() =>
    api.get<{ data: BackupRow[] }>('/admin/backups'),
  );
  const [walship] = createResource(() =>
    auth.isSystemAdmin()
      ? api.get<Record<string, unknown>>('/admin/walship').catch(() => ({ enabled: false }))
      : Promise.resolve(null),
  );

  const [srcUrl, setSrcUrl] = createSignal('');
  const [tokenId, setTokenId] = createSignal('');
  const [tokenSecret, setTokenSecret] = createSignal('');
  const [rpm, setRpm] = createSignal('90');
  const [notice, setNotice] = createSignal<string | null>(null);

  // Live progress while any job runs.
  const timer = setInterval(() => {
    if (imports()?.data?.some(j => j.status === 'running')) refetchImports();
    if (backups()?.data?.some(b => b.status === 'running')) refetchBackups();
  }, 3000);
  onCleanup(() => clearInterval(timer));

  const startImport = async (e: Event) => {
    e.preventDefault();
    setNotice(null);
    try {
      await api.post('/admin/import', {
        base_url: srcUrl(),
        token_id: tokenId(),
        token_secret: tokenSecret(),
        org_id: orgId(),
        rate_limit_per_minute: Number(rpm()) || 90,
      });
      setSrcUrl('');
      setTokenId('');
      setTokenSecret('');
      await refetchImports();
    } catch (err) {
      setNotice(err instanceof Error ? err.message : 'import failed to start');
    }
  };

  const startBackup = async (scope: string, target: string) => {
    setNotice(null);
    try {
      await api.post('/admin/backups', {
        scope,
        target,
        org_id: scope === 'org' ? orgId() : undefined,
      });
      await refetchBackups();
    } catch (err) {
      setNotice(err instanceof Error ? err.message : 'backup failed to start');
    }
  };

  const verify = async (id: number) => {
    setNotice(null);
    try {
      const report = await api.post<Record<string, unknown>>(`/admin/backups/${id}/verify`);
      setNotice(`Backup #${id} verified: ${JSON.stringify(report)}`);
    } catch (err) {
      setNotice(err instanceof Error ? err.message : 'verify failed');
    }
  };

  return (
    <div class="admin-sections">
      <Show when={notice()}>{message => <div class="form-error">{message()}</div>}</Show>

      <section class="admin-card">
        <h2>Import from a BookStack instance</h2>
        <p class="muted-note">
          Pulls shelves, books, chapters and pages (markdown via the export API) into{' '}
          <strong>{auth.activeOrg()?.name}</strong>, pacing requests to respect the source's API
          rate limit and backing off on 429 responses. Images, attachments and drafts are skipped.
        </p>
        <form class="inline-form" onSubmit={startImport}>
          <input placeholder="https://docs.example.com" value={srcUrl()} onInput={e => setSrcUrl(e.currentTarget.value)} required />
          <input placeholder="API token ID" value={tokenId()} onInput={e => setTokenId(e.currentTarget.value)} required />
          <input placeholder="API token secret" type="password" value={tokenSecret()} onInput={e => setTokenSecret(e.currentTarget.value)} required />
          <input placeholder="req/min" style={{ 'max-width': '90px', flex: '0 0 auto' }} value={rpm()} onInput={e => setRpm(e.currentTarget.value)} />
          <button class="btn btn-primary" type="submit">Start import</button>
        </form>
        <For each={imports()?.data}>
          {job => (
            <div class="job-row">
              <span class={`status-pill status-${job.status}`}>{job.status}</span>
              <span class="mono small">#{job.id} {job.source_url}</span>
              <span class="small muted-note">{JSON.stringify(job.progress)}</span>
            </div>
          )}
        </For>
      </section>

      <section class="admin-card">
        <h2>Backups</h2>
        <p class="muted-note">
          Always encrypted (XChaCha20-Poly1305, key from BACKUP_ENCRYPTION_KEY). Per-org backups
          go to object storage; global and SQL backups may also target the server filesystem.
        </p>
        <div class="btn-row" style={{ 'flex-wrap': 'wrap' }}>
          <button class="btn" onClick={() => startBackup('org', 'object_storage')}>
            Backup this org → object storage
          </button>
          <Show when={auth.isSystemAdmin()}>
            <button class="btn" onClick={() => startBackup('global', 'object_storage')}>
              Global → object storage
            </button>
            <button class="btn" onClick={() => startBackup('global', 'filesystem')}>
              Global → filesystem
            </button>
            <button class="btn" onClick={() => startBackup('sql', 'filesystem')}>
              SQL dump → filesystem
            </button>
          </Show>
        </div>
        <table class="admin-table">
          <thead>
            <tr><th>#</th><th>Scope</th><th>Target</th><th>Size</th><th>Status</th><th /></tr>
          </thead>
          <tbody>
            <For each={backups()?.data}>
              {row => (
                <tr>
                  <td>{row.id}</td>
                  <td>{row.scope}{row.org_id ? ` (org ${row.org_id})` : ''}</td>
                  <td>{row.target}</td>
                  <td>{(row.size_bytes / 1024).toFixed(1)} KB</td>
                  <td><span class={`status-pill status-${row.status}`}>{row.status}</span>{row.error ? ` ${row.error}` : ''}</td>
                  <td>
                    <Show when={row.status === 'completed'}>
                      <button class="btn btn-tiny" onClick={() => verify(row.id)}>verify</button>
                    </Show>
                  </td>
                </tr>
              )}
            </For>
          </tbody>
        </table>
      </section>

      <Show when={auth.isSystemAdmin()}>
        <section class="admin-card">
          <h2>Realtime WAL shipping</h2>
          <p class="muted-note">
            Streams every database change (Postgres WAL segments via a replication slot) encrypted
            to object storage or the filesystem as segments complete. Configure with
            WALSHIP_ENABLED=true, WALSHIP_TARGET, BACKUP_ENCRYPTION_KEY.
          </p>
          <pre class="mono small">{JSON.stringify(walship() ?? {}, null, 2)}</pre>
        </section>
      </Show>
    </div>
  );
}

function OrgTab() {
  const auth = useAuth();
  const orgId = () => auth.activeOrg()?.org_id ?? 0;

  const [settings, { refetch: refetchSettings }] = createResource(orgId, id =>
    api.get<{ inherit_global_auth: boolean; can_manage_providers: boolean }>(`/orgs/${id}/settings`),
  );
  const [members, { refetch: refetchMembers }] = createResource(orgId, id =>
    api.get<{ data: OrgMemberInfo[] }>(`/orgs/${id}/members`),
  );
  const [providers, { refetch: refetchProviders }] = createResource(orgId, id =>
    api.get<{ data: AuthProvider[] }>(`/orgs/${id}/auth-providers`),
  );

  const [email, setEmail] = createSignal('');
  const [role, setRole] = createSignal('viewer');
  const [newOrgName, setNewOrgName] = createSignal('');

  const addMember = async (e: Event) => {
    e.preventDefault();
    await api.post(`/orgs/${orgId()}/members`, { email: email(), role: role() });
    setEmail('');
    await refetchMembers();
  };

  const removeMember = async (userId: number) => {
    if (!confirm('Remove this member from the org?')) return;
    await api.delete(`/orgs/${orgId()}/members/${userId}`);
    await refetchMembers();
  };

  const toggleInherit = async (value: boolean) => {
    await api.put(`/orgs/${orgId()}/settings`, { inherit_global_auth: value });
    await refetchSettings();
  };

  const createOrg = async (e: Event) => {
    e.preventDefault();
    await api.post('/orgs', { name: newOrgName() });
    setNewOrgName('');
    await auth.refreshOrgs();
    alert('Org created — switch to it from the org menu in the header.');
  };

  return (
    <div class="admin-sections">
      <section class="admin-card">
        <h2>Members</h2>
        <form class="inline-form" onSubmit={addMember}>
          <input
            type="email"
            placeholder="user@example.com"
            value={email()}
            onInput={e => setEmail(e.currentTarget.value)}
            required
          />
          <select value={role()} onChange={e => setRole(e.currentTarget.value)}>
            <option value="viewer">viewer</option>
            <option value="editor">editor</option>
            <option value="admin">admin</option>
          </select>
          <button class="btn btn-primary" type="submit">
            Add member
          </button>
        </form>
        <table class="admin-table">
          <thead>
            <tr>
              <th>Name</th>
              <th>Email</th>
              <th>Role</th>
              <th />
            </tr>
          </thead>
          <tbody>
            <For each={members()?.data}>
              {member => (
                <tr>
                  <td>{member.name}</td>
                  <td>{member.email}</td>
                  <td>
                    <span class={`role-badge role-${member.role}`}>{member.role}</span>
                  </td>
                  <td>
                    <button class="btn btn-tiny btn-danger" onClick={() => removeMember(member.user_id)}>
                      remove
                    </button>
                  </td>
                </tr>
              )}
            </For>
          </tbody>
        </table>
      </section>

      <section class="admin-card">
        <h2>Authentication</h2>
        <label class="check-row">
          <input
            type="checkbox"
            checked={settings()?.inherit_global_auth ?? true}
            onChange={e => toggleInherit(e.currentTarget.checked)}
          />
          Inherit global sign-in providers
        </label>
        <Show
          when={settings()?.can_manage_providers}
          fallback={
            <p class="muted-note">
              Org-level sign-in providers are disabled by the instance administrator. This org
              inherits the global providers{settings()?.inherit_global_auth === false ? ' (currently opted out)' : ''}.
            </p>
          }
        >
          <ProviderManager
            providers={() => providers()?.data ?? []}
            basePath={() => `/orgs/${orgId()}/auth-providers`}
            onChanged={refetchProviders}
          />
        </Show>
      </section>

      <section class="admin-card">
        <h2>Branding</h2>
        <p class="muted-note">
          Logo and colors for <strong>{auth.activeOrg()?.name}</strong>. Unset fields inherit the
          global branding.
        </p>
        <BrandingCard path={() => `/orgs/${orgId()}/branding`} onSaved={() => loadBranding(orgId())} />
      </section>

      <section class="admin-card">
        <h2>New organization</h2>
        <p class="muted-note">Create a separate org — you become its admin.</p>
        <form class="inline-form" onSubmit={createOrg}>
          <input
            placeholder="Org name"
            value={newOrgName()}
            onInput={e => setNewOrgName(e.currentTarget.value)}
            required
          />
          <button class="btn btn-primary" type="submit">
            Create org
          </button>
        </form>
      </section>
    </div>
  );
}

function GlobalTab() {
  const [settings, { refetch: refetchSettings }] = createResource(() =>
    api.get<{ orgs_can_manage_auth: boolean }>('/admin/settings'),
  );
  const [providers, { refetch: refetchProviders }] = createResource(() =>
    api.get<{ data: AuthProvider[] }>('/admin/auth-providers'),
  );

  const toggleOrgsCanManage = async (value: boolean) => {
    await api.put('/admin/settings', { orgs_can_manage_auth: value });
    await refetchSettings();
  };

  return (
    <div class="admin-sections">
      <section class="admin-card">
        <h2>Global authentication policy</h2>
        <label class="check-row">
          <input
            type="checkbox"
            checked={settings()?.orgs_can_manage_auth ?? false}
            onChange={e => toggleOrgsCanManage(e.currentTarget.checked)}
          />
          Allow organizations to add and edit their own sign-in providers
        </label>
        <p class="muted-note">
          When unchecked, orgs inherit the global providers below and cannot define their own
          (system admins can still manage any org's providers).
        </p>
      </section>

      <section class="admin-card">
        <h2>Global branding</h2>
        <p class="muted-note">Default logo and colors for every org (and the login page) unless an org overrides them.</p>
        <BrandingCard path={() => '/admin/branding'} onSaved={() => loadBranding(null)} />
      </section>

      <section class="admin-card">
        <h2>Global sign-in providers</h2>
        <p class="muted-note">Available on every org's login unless the org opts out of inheritance.</p>
        <ProviderManager
          providers={() => providers()?.data ?? []}
          basePath={() => '/admin/auth-providers'}
          onChanged={refetchProviders}
        />
      </section>
    </div>
  );
}

function ProviderManager(props: {
  providers: () => AuthProvider[];
  basePath: () => string;
  onChanged: () => void;
}) {
  const empty = {
    name: '',
    client_id: '',
    client_secret: '',
    authorize_url: '',
    token_url: '',
    userinfo_url: '',
    scopes: 'openid profile email',
  };
  const [form, setForm] = createSignal({ ...empty });
  const [editing, setEditing] = createSignal<number | null>(null);
  const [error, setError] = createSignal<string | null>(null);

  const startEdit = (provider: AuthProvider) => {
    setEditing(provider.id);
    setForm({
      name: provider.name,
      client_id: provider.client_id,
      client_secret: '',
      authorize_url: provider.authorize_url,
      token_url: provider.token_url,
      userinfo_url: provider.userinfo_url,
      scopes: provider.scopes,
    });
  };

  const submit = async (e: Event) => {
    e.preventDefault();
    setError(null);
    try {
      if (editing() !== null) {
        await api.put(`${props.basePath()}/${editing()}`, form());
      } else {
        await api.post(props.basePath(), form());
      }
      setForm({ ...empty });
      setEditing(null);
      props.onChanged();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'save failed');
    }
  };

  const remove = async (id: number) => {
    if (!confirm('Delete this sign-in provider?')) return;
    await api.delete(`${props.basePath()}/${id}`);
    props.onChanged();
  };

  return (
    <div>
      <table class="admin-table">
        <thead>
          <tr>
            <th>Name</th>
            <th>Authorize URL</th>
            <th>Enabled</th>
            <th />
          </tr>
        </thead>
        <tbody>
          <For each={props.providers()} fallback={<tr><td colspan="4" class="muted-note">No providers yet.</td></tr>}>
            {provider => (
              <tr>
                <td>{provider.name}</td>
                <td class="mono small">{provider.authorize_url}</td>
                <td>{provider.enabled ? '✓' : '—'}</td>
                <td>
                  <button class="btn btn-tiny" onClick={() => startEdit(provider)}>
                    edit
                  </button>{' '}
                  <button class="btn btn-tiny btn-danger" onClick={() => remove(provider.id)}>
                    delete
                  </button>
                </td>
              </tr>
            )}
          </For>
        </tbody>
      </table>

      <form class="provider-form" onSubmit={submit}>
        <h3>{editing() !== null ? 'Edit provider' : 'Add provider'}</h3>
        <div class="provider-grid">
          <input placeholder="Display name (e.g. Okta)" value={form().name} onInput={e => setForm({ ...form(), name: e.currentTarget.value })} required />
          <input placeholder="Client ID" value={form().client_id} onInput={e => setForm({ ...form(), client_id: e.currentTarget.value })} required />
          <input placeholder={editing() !== null ? 'Client secret (blank = keep)' : 'Client secret'} value={form().client_secret} onInput={e => setForm({ ...form(), client_secret: e.currentTarget.value })} />
          <input placeholder="Scopes" value={form().scopes} onInput={e => setForm({ ...form(), scopes: e.currentTarget.value })} />
          <input placeholder="Authorize URL" value={form().authorize_url} onInput={e => setForm({ ...form(), authorize_url: e.currentTarget.value })} required />
          <input placeholder="Token URL" value={form().token_url} onInput={e => setForm({ ...form(), token_url: e.currentTarget.value })} required />
          <input placeholder="Userinfo URL" value={form().userinfo_url} onInput={e => setForm({ ...form(), userinfo_url: e.currentTarget.value })} required />
        </div>
        <Show when={error()}>{message => <div class="form-error">{message()}</div>}</Show>
        <div class="btn-row">
          <button class="btn btn-primary" type="submit">
            {editing() !== null ? 'Save changes' : 'Add provider'}
          </button>
          <Show when={editing() !== null}>
            <button
              class="btn btn-ghost-dark"
              type="button"
              onClick={() => {
                setEditing(null);
                setForm({ ...empty });
              }}
            >
              Cancel
            </button>
          </Show>
        </div>
      </form>
    </div>
  );
}


interface BrandingScope {
  name?: string;
  primary_color?: string;
  primary_dark_color?: string;
  header_text_color?: string;
}

function BrandingCard(props: { path: () => string; onSaved: () => void }) {
  const [scope, { refetch }] = createResource(props.path, p => api.get<BrandingScope>(p));
  const [name, setName] = createSignal('');
  const [primary, setPrimary] = createSignal('#206ea7');
  const [primaryDark, setPrimaryDark] = createSignal('#0f4d79');
  const [headerText, setHeaderText] = createSignal('#ffffff');
  const [notice, setNotice] = createSignal<string | null>(null);

  createEffect(() => {
    const current = scope();
    if (!current) return;
    setName(current.name ?? '');
    setPrimary(current.primary_color ?? '#206ea7');
    setPrimaryDark(current.primary_dark_color ?? '#0f4d79');
    setHeaderText(current.header_text_color ?? '#ffffff');
  });

  const save = async (e: Event) => {
    e.preventDefault();
    setNotice(null);
    try {
      await api.put(props.path(), {
        name: name(),
        primary_color: primary(),
        primary_dark_color: primaryDark(),
        header_text_color: headerText(),
      });
      await refetch();
      props.onSaved();
      setNotice('Saved.');
    } catch (err) {
      setNotice(err instanceof Error ? err.message : 'save failed');
    }
  };

  const uploadLogo = async (e: Event) => {
    const input = e.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    setNotice(null);
    try {
      const res = await fetch(`/api${props.path()}/logo`, {
        method: 'POST',
        headers: {
          Authorization: `Bearer ${getToken()}`,
          'Content-Type': file.type || 'image/png',
        },
        body: file,
      });
      if (!res.ok) {
        const data = await res.json().catch(() => null);
        throw new Error(data?.error?.message ?? `upload failed (${res.status})`);
      }
      props.onSaved();
      setNotice('Logo uploaded.');
    } catch (err) {
      setNotice(err instanceof Error ? err.message : 'upload failed');
    } finally {
      input.value = '';
    }
  };

  const removeLogo = async () => {
    setNotice(null);
    await api.delete(`${props.path()}/logo`);
    props.onSaved();
    setNotice('Logo removed.');
  };

  return (
    <form onSubmit={save}>
      <div class="branding-grid">
        <label>
          Display name
          <input type="text" placeholder="BookStack" value={name()} onInput={e => setName(e.currentTarget.value)} />
        </label>
        <label>
          Primary color
          <input type="color" value={primary()} onInput={e => setPrimary(e.currentTarget.value)} />
        </label>
        <label>
          Primary dark
          <input type="color" value={primaryDark()} onInput={e => setPrimaryDark(e.currentTarget.value)} />
        </label>
        <label>
          Header text
          <input type="color" value={headerText()} onInput={e => setHeaderText(e.currentTarget.value)} />
        </label>
      </div>
      <Show when={notice()}>{message => <p class="muted-note">{message()}</p>}</Show>
      <div class="btn-row" style={{ 'flex-wrap': 'wrap' }}>
        <button class="btn btn-primary" type="submit">Save branding</button>
        <label class="btn" style={{ cursor: 'pointer' }}>
          Upload logo (png/jpeg/webp, ≤512KB)
          <input type="file" accept="image/png,image/jpeg,image/webp" style={{ display: 'none' }} onChange={uploadLogo} />
        </label>
        <button class="btn btn-danger" type="button" onClick={removeLogo}>Remove logo</button>
      </div>
    </form>
  );
}
