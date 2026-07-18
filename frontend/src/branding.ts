import { createSignal } from 'solid-js';

export interface Branding {
  name: string;
  primary_color: string;
  primary_dark_color: string;
  header_text_color: string;
  has_logo: boolean;
  logo_scope: 'org' | 'global' | null;
}

const DEFAULTS: Branding = {
  name: 'BookStack',
  primary_color: '#206ea7',
  primary_dark_color: '#0f4d79',
  header_text_color: '#ffffff',
  has_logo: false,
  logo_scope: null,
};

const [branding, setBranding] = createSignal<Branding>(DEFAULTS);
export { branding };

/// Push branding into the CSS custom properties driving the whole UI.
function applyCssVars(b: Branding) {
  const root = document.documentElement;
  root.style.setProperty('--primary', b.primary_color);
  root.style.setProperty('--primary-dark', b.primary_dark_color);
  root.style.setProperty('--header-text', b.header_text_color);
}

/// Fetch + apply effective branding for a surface (org page or, with no
/// org, the global/login default). Public endpoint — no auth needed.
export async function loadBranding(orgId: number | null): Promise<void> {
  try {
    const query = orgId ? `?org_id=${orgId}` : '';
    const res = await fetch(`/api/branding${query}`);
    if (!res.ok) throw new Error(String(res.status));
    const data = (await res.json()) as Branding;
    const merged = { ...DEFAULTS, ...data };
    setBranding(merged);
    applyCssVars(merged);
  } catch {
    setBranding(DEFAULTS);
    applyCssVars(DEFAULTS);
  }
}

export function logoUrl(orgId: number | null): string {
  return orgId ? `/api/branding/logo?org_id=${orgId}` : '/api/branding/logo';
}
