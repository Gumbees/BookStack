import { createResource, For, Show } from 'solid-js';
import { A, useSearchParams } from '@solidjs/router';
import { api } from '../api';
import type { Paginated, SearchResult } from '../types';

const ICONS: Record<SearchResult['entity_type'], string> = {
  page: '📄',
  book: '📘',
  chapter: '📑',
  shelf: '🗄',
};

function resultHref(result: SearchResult): string {
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
  const [results] = createResource(
    () => (typeof searchParams.query === 'string' ? searchParams.query : ''),
    query =>
      query
        ? api.get<Paginated<SearchResult>>(`/search?query=${encodeURIComponent(query)}&count=50`)
        : Promise.resolve({ data: [], total: 0 }),
  );

  return (
    <div>
      <h1>Search</h1>
      <p class="page-desc">
        Results for “{searchParams.query}” — {results()?.total ?? 0} match
        {(results()?.total ?? 0) === 1 ? '' : 'es'}
      </p>
      <ul class="search-results">
        <For each={results()?.data} fallback={<div class="empty-note">No results.</div>}>
          {result => (
            <li class="search-result">
              <A href={resultHref(result)}>
                <div class="search-result-title">
                  <span>{ICONS[result.entity_type]}</span>
                  <strong>{result.name}</strong>
                  <span class="entity-badge">{result.entity_type}</span>
                </div>
                <div class="search-preview" innerHTML={result.preview} />
              </A>
            </li>
          )}
        </For>
      </ul>
    </div>
  );
}
