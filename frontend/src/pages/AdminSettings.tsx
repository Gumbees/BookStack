import { createResource, createSignal, For, Show } from 'solid-js';
import { api } from '../api';
import { useAuth } from '../auth';
import type { AuthProvider, OrgMemberInfo } from '../types';

/// Admin settings: an org tab (members + org auth providers, gated by the
/// global checkbox) and a global tab for system admins (the checkbox itself
/// plus instance-wide auth providers, inherited by every org by default).
export default function AdminSettings() {
  const auth = useAuth();
  const [tab, setTab] = createSignal<'org' | 'global'>('org');

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
      </div>
      <Show when={tab() === 'org'}>
        <OrgTab />
      </Show>
      <Show when={tab() === 'global'}>
        <GlobalTab />
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
