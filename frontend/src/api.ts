import type { OrgMembership, User } from './types';

const TOKEN_KEY = 'bookstack_token';
const USER_KEY = 'bookstack_user';
const ORGS_KEY = 'bookstack_orgs';
const ACTIVE_ORG_KEY = 'bookstack_active_org';

export function getToken(): string | null {
  return localStorage.getItem(TOKEN_KEY);
}

export function getStoredUser(): User | null {
  const raw = localStorage.getItem(USER_KEY);
  if (!raw) return null;
  try {
    return JSON.parse(raw) as User;
  } catch {
    return null;
  }
}

export function getStoredOrgs(): OrgMembership[] {
  const raw = localStorage.getItem(ORGS_KEY);
  if (!raw) return [];
  try {
    return JSON.parse(raw) as OrgMembership[];
  } catch {
    return [];
  }
}

export function getActiveOrgId(): number | null {
  const raw = localStorage.getItem(ACTIVE_ORG_KEY);
  const parsed = raw ? Number(raw) : NaN;
  return Number.isFinite(parsed) ? parsed : null;
}

export function setActiveOrgId(orgId: number) {
  localStorage.setItem(ACTIVE_ORG_KEY, String(orgId));
}

export function storeSession(token: string, user: User, orgs: OrgMembership[]) {
  localStorage.setItem(TOKEN_KEY, token);
  localStorage.setItem(USER_KEY, JSON.stringify(user));
  localStorage.setItem(ORGS_KEY, JSON.stringify(orgs));
  const active = getActiveOrgId();
  if (!active || !orgs.some(o => o.org_id === active)) {
    if (orgs.length > 0) setActiveOrgId(orgs[0].org_id);
  }
}

export function clearSession() {
  localStorage.removeItem(TOKEN_KEY);
  localStorage.removeItem(USER_KEY);
  localStorage.removeItem(ORGS_KEY);
  localStorage.removeItem(ACTIVE_ORG_KEY);
}

export class ApiError extends Error {
  status: number;
  constructor(status: number, message: string) {
    super(message);
    this.status = status;
  }
}

async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
  const headers: Record<string, string> = {};
  const token = getToken();
  if (token) headers['Authorization'] = `Bearer ${token}`;
  const activeOrg = getActiveOrgId();
  if (activeOrg) headers['X-Org-Id'] = String(activeOrg);
  if (body !== undefined) headers['Content-Type'] = 'application/json';

  const res = await fetch(`/api${path}`, {
    method,
    headers,
    body: body === undefined ? undefined : JSON.stringify(body),
  });

  if (res.status === 401) {
    clearSession();
    if (!location.pathname.startsWith('/login')) {
      location.assign('/login');
    }
    throw new ApiError(401, 'unauthorized');
  }
  if (!res.ok) {
    let message = res.statusText;
    try {
      const data = await res.json();
      message = data?.error?.message ?? message;
    } catch {
      /* keep statusText */
    }
    throw new ApiError(res.status, message);
  }
  return (await res.json()) as T;
}

export const api = {
  get: <T>(path: string) => request<T>('GET', path),
  post: <T>(path: string, body?: unknown) => request<T>('POST', path, body),
  put: <T>(path: string, body?: unknown) => request<T>('PUT', path, body),
  delete: <T>(path: string) => request<T>('DELETE', path),
};

/// Base WebSocket URL for collaboration rooms; y-websocket appends `/<room>`
/// (the page id) and the auth token via `params`.
export function collabServerBase(): string {
  const proto = location.protocol === 'https:' ? 'wss' : 'ws';
  return `${proto}://${location.host}/ws/pages`;
}

const PALETTE = ['#1e88e5', '#43a047', '#e53935', '#8e24aa', '#f4511e', '#00897b', '#3949ab', '#c0ca33'];

export function userColor(id: number): string {
  return PALETTE[Math.abs(id) % PALETTE.length];
}
