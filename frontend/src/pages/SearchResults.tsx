import { createResource, For, Show } from 'solid-js';
import { A, useSearchParams } from '@solidjs/router';
import { api, getActiveOrgId, setActiveOrgId } from '../api';
import type { Paginated, SearchResult, SemanticResponse } from '../types';

const ICONS: Record<SearchResult['entity_type'], string> = {
  page: '📄',
  book: '📘',
  chapter: '📑',
  shelf: '🗄',
};

interface UnifiedResult {
  entity_type: SearchResult['entity_type'];
  org_id: number;
  org_slug: string;
  name: string;
  slug: string;
  book_slug: string | null;
  preview: string;
  previewIsHtml: boolean;
  score?: number;
}

function resultHref(result: UnifiedResult): string {
  switch (result.entity_type) {
    case 'page':
      return `/book/${result.book_slug}/page/${result.slug}`;
    case 'chapter':
      return `/book/${result.book_slug}`;
    case 'book':
      return `/book/${result.slug}`;
    case 'shelf':
      return `/shelf/${result.slug}`;
  }
}

export default function SearchResults() {
  const [searchParams] = useSearchParams();
  const query = () => (typeof searchParams.query === 'string' ? searchParams.query : '');
  const mode = () => (typeof searchParams.mode === 'string' ? searchParams.mode : 'keyword');
  const scope = () => (typeof searchParams.scope === 'string' ? searchParams.scope : 'org');

  const [results] = createResource(
    () => `${query()}|${mode()}|${scope()}`,
    async (): Promise<{ items: UnifiedResult[]; total: number; note?: string }> => {
      const q = query();
      if (!q) return { items: [], total: 0 };
      const scopeParam = scope() === 'global' ? '&scope=global' : '';
      if (mode() === 'keyword') {
        const res = await api.get<Paginated<SearchResult>>(
          `/search?query=${encodeURIComponent(q)}&count=50${scopeParam}`,
        );
        return {
          total: res.total,
          items: res.data.map(r => ({
            entity_type: r.entity_type,
            org_id: r.org_id,
            org_slug: r.org_slug,
            name: r.name,
            slug: r.slug,
            book_slug: r.book_slug,
            preview: r.preview,
            previewIsHtml: true,
          })),
        };
      }
      try {
        const res = await api.get<SemanticResponse>(
          `/search/semantic?query=${encodeURIComponent(q)}&mode=${mode()}&count=30${scopeParam}`,
        );
        return {
          total: res.results.length,
          items: res.results.map(r => ({
            entity_type: r.entity_type,
            org_id: r.org_id,
            org_slug: r.org_slug,
            name: r.name,
            slug: r.slug,
            book_slug: r.book_slug,
            preview: r.chunks[0]?.content ?? '',
            previewIsHtml: false,
            score: r.score,
          })),
        };
      } catch (err) {
        return {
          items: [],
          total: 0,
          note: err instanceof Error ? err.message : 'semantic search unavailable',
        };
      }
    },
  );

  return (
    <div>
      <h1>Search</h1>
      <p class="page-desc">
        Results for “{query()}” · mode: <strong>{mode()}</strong>
        {scope() === 'global' ? ' · all orgs' : ''} — {results()?.total ?? 0} match
        {(results()?.total ?? 0) === 1 ? '' : 'es'}
      </p>
      <Show when={results()?.note}>
        {note => <div class="form-error">{note()}</div>}
      </Show>
      <ul class="search-results">
        <For each={results()?.items} fallback={<div class="empty-note">No results.</div>}>
          {result => (
            <li class="search-result">
              <A
                href={resultHref(result)}
                onClick={e => {
                  // Cross-org result: switch the active org, then hard-navigate
                  // so every org-scoped resource reloads in the right context.
                  if (result.org_id !== getActiveOrgId()) {
                    e.preventDefault();
                    setActiveOrgId(result.org_id);
                    location.assign(resultHref(result));
                  }
                }}
              >
                <div class="search-result-title">
                  <span>{ICONS[result.entity_type]}</span>
                  <strong>{result.name}</strong>
                  <span class="entity-badge">{result.entity_type}</span>
                  <Show when={scope() === 'global'}>
                    <span class="entity-badge org-badge">🏢 {result.org_slug}</span>
                  </Show>
                  <Show when={result.score !== undefined}>
                    <span class="score-badge">{(result.score! * 100).toFixed(0)}%</span>
                  </Show>
                </div>
                <Show
                  when={result.previewIsHtml}
                  fallback={<div class="search-preview">{result.preview}</div>}
                >
                  <div class="search-preview" innerHTML={result.preview} />
                </Show>
              </A>
            </li>
          )}
        </For>
      </ul>
    </div>
  );
}
