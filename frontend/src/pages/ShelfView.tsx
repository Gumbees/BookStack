import { createResource, For, Show } from 'solid-js';
import { A, useParams } from '@solidjs/router';
import { api } from '../api';
import type { ShelfDetails } from '../types';

export default function ShelfView() {
  const params = useParams();
  const [shelf] = createResource(
    () => params.slug,
    slug => api.get<ShelfDetails>(`/shelves/slug/${slug}`),
  );

  return (
    <Show when={shelf()} fallback={<div class="empty-note">Loading…</div>}>
      {s => (
        <div>
          <nav class="crumbs">
            <A href="/">Home</A> <span>/</span> <span>{s().name}</span>
          </nav>
          <h1>🗄 {s().name}</h1>
          <p class="page-desc">{s().description}</p>
          <div class="card-grid">
            <For each={s().books} fallback={<div class="empty-note">No books on this shelf.</div>}>
              {book => (
                <A href={`/book/${book.slug}`} class="card book-card">
                  <div class="card-icon">📘</div>
                  <div>
                    <div class="card-title">{book.name}</div>
                    <div class="card-sub">{book.description || 'Book'}</div>
                  </div>
                </A>
              )}
            </For>
          </div>
        </div>
      )}
    </Show>
  );
}
