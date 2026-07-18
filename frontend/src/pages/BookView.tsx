import { createResource, createSignal, For, Show } from 'solid-js';
import { A, useNavigate, useParams } from '@solidjs/router';
import { api } from '../api';
import { useAuth } from '../auth';
import type { BookDetails, ContentItem, Page } from '../types';

type ChapterDetailsResponse = { id: number };

export default function BookView() {
  const params = useParams();
  const auth = useAuth();
  const navigate = useNavigate();
  const [book, { refetch }] = createResource(
    () => params.slug,
    slug => api.get<BookDetails>(`/books/slug/${slug}`),
  );

  const [newPageName, setNewPageName] = createSignal('');
  const [newChapterName, setNewChapterName] = createSignal('');
  const [showNewPage, setShowNewPage] = createSignal<number | null | false>(false);
  const [showNewChapter, setShowNewChapter] = createSignal(false);

  const createPage = async (e: Event, chapterId: number | null) => {
    e.preventDefault();
    const b = book();
    if (!b) return;
    const created = await api.post<Page>('/pages', {
      book_id: b.id,
      chapter_id: chapterId,
      name: newPageName(),
      markdown: '',
    });
    setNewPageName('');
    setShowNewPage(false);
    navigate(`/book/${b.slug}/page/${created.slug}/edit`);
  };

  const createChapter = async (e: Event) => {
    e.preventDefault();
    const b = book();
    if (!b) return;
    await api.post<ChapterDetailsResponse>('/chapters', { book_id: b.id, name: newChapterName() });
    setNewChapterName('');
    setShowNewChapter(false);
    await refetch();
  };

  const deleteBook = async () => {
    const b = book();
    if (!b) return;
    if (!confirm(`Delete book "${b.name}" and all of its contents?`)) return;
    await api.delete(`/books/${b.id}`);
    navigate('/');
  };

  return (
    <Show when={book()} fallback={<div class="empty-note">Loading…</div>}>
      {b => (
        <div>
          <nav class="crumbs">
            <A href="/">Home</A> <span>/</span> <span>{b().name}</span>
          </nav>
          <div class="page-head">
            <h1>📘 {b().name}</h1>
            <Show when={auth.canEdit()}>
              <div class="btn-row">
                <button class="btn" onClick={() => setShowNewPage(null)}>
                  + Page
                </button>
                <button class="btn" onClick={() => setShowNewChapter(true)}>
                  + Chapter
                </button>
                <button class="btn btn-danger" onClick={deleteBook}>
                  Delete
                </button>
              </div>
            </Show>
          </div>
          <p class="page-desc">{b().description}</p>

          <Show when={showNewChapter()}>
            <form class="inline-form" onSubmit={createChapter}>
              <input
                placeholder="Chapter name"
                value={newChapterName()}
                onInput={e => setNewChapterName(e.currentTarget.value)}
                required
              />
              <button class="btn btn-primary" type="submit">
                Create chapter
              </button>
              <button class="btn btn-ghost" type="button" onClick={() => setShowNewChapter(false)}>
                Cancel
              </button>
            </form>
          </Show>
          <Show when={showNewPage() !== false}>
            <form class="inline-form" onSubmit={e => createPage(e, showNewPage() as number | null)}>
              <input
                placeholder="Page name"
                value={newPageName()}
                onInput={e => setNewPageName(e.currentTarget.value)}
                required
              />
              <button class="btn btn-primary" type="submit">
                Create & edit
              </button>
              <button class="btn btn-ghost" type="button" onClick={() => setShowNewPage(false)}>
                Cancel
              </button>
            </form>
          </Show>

          <div class="contents-tree">
            <For each={b().contents} fallback={<div class="empty-note">This book is empty.</div>}>
              {item => <ContentNode item={item} bookSlug={b().slug} onNewPage={id => setShowNewPage(id)} />}
            </For>
          </div>
        </div>
      )}
    </Show>
  );
}

function ContentNode(props: {
  item: ContentItem;
  bookSlug: string;
  onNewPage: (chapterId: number) => void;
}) {
  const auth = useAuth();
  return (
    <Show
      when={props.item.type === 'chapter' ? (props.item as Extract<ContentItem, { type: 'chapter' }>) : null}
      fallback={
        <A
          href={`/book/${props.bookSlug}/page/${props.item.slug}`}
          class="tree-row tree-page"
        >
          <span class="tree-icon">📄</span> {props.item.name}
        </A>
      }
    >
      {chapter => (
        <div class="tree-chapter">
          <div class="tree-row tree-chapter-head">
            <span class="tree-icon">📑</span>
            <span class="tree-chapter-name">{chapter().name}</span>
            <Show when={auth.canEdit()}>
              <button class="btn btn-tiny" onClick={() => props.onNewPage(chapter().id)}>
                + page
              </button>
            </Show>
          </div>
          <div class="tree-children">
            <For each={chapter().pages}>
              {page => (
                <A href={`/book/${props.bookSlug}/page/${page.slug}`} class="tree-row tree-page">
                  <span class="tree-icon">📄</span> {page.name}
                </A>
              )}
            </For>
          </div>
        </div>
      )}
    </Show>
  );
}
