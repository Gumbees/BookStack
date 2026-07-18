import { createContext, createSignal, useContext, type ParentProps } from 'solid-js';
import {
  api,
  clearSession,
  getActiveOrgId,
  getStoredOrgs,
  getStoredUser,
  getToken,
  setActiveOrgId,
  storeSession,
} from './api';
import type { OrgMembership, User } from './types';

interface LoginResponse {
  token: string;
  user: User;
  orgs: OrgMembership[];
}

interface AuthContextValue {
  user: () => User | null;
  orgs: () => OrgMembership[];
  activeOrg: () => OrgMembership | null;
  loggedIn: () => boolean;
  canEdit: () => boolean;
  isOrgAdmin: () => boolean;
  isSystemAdmin: () => boolean;
  login: (email: string, password: string) => Promise<void>;
  /// Accept a JWT minted by the SSO callback (#sso_token=…).
  adoptSsoToken: (token: string) => Promise<void>;
  switchOrg: (orgId: number) => void;
  refreshOrgs: () => Promise<void>;
  logout: () => void;
}

const AuthContext = createContext<AuthContextValue>();

export function AuthProvider(props: ParentProps) {
  const [user, setUser] = createSignal<User | null>(getStoredUser());
  const [orgs, setOrgs] = createSignal<OrgMembership[]>(getStoredOrgs());

  const activeOrg = () => {
    const id = getActiveOrgId();
    const list = orgs();
    return list.find(o => o.org_id === id) ?? list[0] ?? null;
  };

  const value: AuthContextValue = {
    user,
    orgs,
    activeOrg,
    loggedIn: () => !!getToken() && !!user(),
    canEdit: () => {
      const u = user();
      if (u?.role === 'admin') return true;
      const org = activeOrg();
      return !!org && (org.role === 'admin' || org.role === 'editor');
    },
    isOrgAdmin: () => {
      const u = user();
      if (u?.role === 'admin') return true;
      return activeOrg()?.role === 'admin';
    },
    isSystemAdmin: () => user()?.role === 'admin',
    login: async (email, password) => {
      const res = await api.post<LoginResponse>('/auth/login', { email, password });
      storeSession(res.token, res.user, res.orgs);
      setUser(res.user);
      setOrgs(res.orgs);
    },
    adoptSsoToken: async (token: string) => {
      localStorage.setItem('bookstack_token', token);
      const res = await api.get<{ user: User; orgs: OrgMembership[] }>('/auth/me');
      storeSession(token, res.user, res.orgs);
      setUser(res.user);
      setOrgs(res.orgs);
    },
    switchOrg: (orgId: number) => {
      setActiveOrgId(orgId);
      // Every resource on screen is org-scoped; a clean reload is the
      // simplest correct refresh.
      location.assign('/');
    },
    refreshOrgs: async () => {
      const res = await api.get<{ user: User; orgs: OrgMembership[] }>('/auth/me');
      storeSession(getToken() ?? '', res.user, res.orgs);
      setUser(res.user);
      setOrgs(res.orgs);
    },
    logout: () => {
      clearSession();
      setUser(null);
      setOrgs([]);
    },
  };

  return <AuthContext.Provider value={value}>{props.children}</AuthContext.Provider>;
}

export function useAuth(): AuthContextValue {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error('useAuth outside AuthProvider');
  return ctx;
}
