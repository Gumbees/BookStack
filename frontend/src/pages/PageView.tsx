import { createResource, createSignal, onCleanup, onMount, Show } from 'solid-js';
import { A, useNavigate, useParams } from '@solidjs/router';
import { api } from '../api';
import { useAuth } from '../auth';
import type { Page } from '../types';

export default function PageView() {
  const params = useParams();
  const auth = useAuth();
  const navigate = useNavigate();
  const [page] = createResource(
    () => `${params.bookSlug}/${params.pageSlug}`,
    () => api.get<Page>(`/pages/by-slugs/${params.bookSlug}/${params.pageSlug}`),
  );
  const [editors, setEditors] = createSignal(0);

  onMount(() => {
    const poll = async () => {
      const p = page();
      if (!p) return;
      try {
        const res = await api.get<{ editors: number }>(`/pages/${p.id}/collab/editors`);
        setEditors(res.editors);
      } catch {
        /* non-fatal */
      }
    };
    const timer = setInterval(poll, 5000);
    const initial = setTimeout(poll, 400);
    onCleanup(() => {
      clearInterval(timer);
      clearTimeout(initial);
    });
  });

  const deletePage = async () => {
    const p = page();
    if (!p) return;
    if (!confirm(`Delete page "${p.name}"?`)) return;
    await api.delete(`/pages/${p.id}`);
    navigate(`/book/${params.bookSlug}`);
  };

  return (
    <Show when={page()} fallback={<div class="empty-note">Loading…</div>}>
      {p => (
        <article class="page-article">
          <nav class="crumbs">
            <A href="/">Home</A> <span>/</span>{' '}
            <A href={`/book/${params.bookSlug}`}>{params.bookSlug}</A> <span>/</span>{' '}
            <span>{p().name}</span>
          </nav>
          <div class="page-head">
            <h1>{p().name}</h1>
            <div class="btn-row">
              <Show when={editors() > 0}>
                <span class="live-badge" title="People in the collaborative editor right now">
                  ● {editors()} editing live
                </span>
              </Show>
              <Show when={auth.canEdit()}>
                <A class="btn btn-primary" href={`/book/${params.bookSlug}/page/${params.pageSlug}/edit`}>
                  Edit
                </A>
                <button class="btn btn-danger" onClick={deletePage}>
                  Delete
                </button>
              </Show>
            </div>
          </div>
          <div class="page-meta">
            Revision #{p().revision_count} · updated {new Date(p().updated_at).toLocaleString()}
          </div>
          <div class="page-content" innerHTML={p().html} />
        </article>
      )}
    </Show>
  );
}
