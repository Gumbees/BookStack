import { createResource, createSignal, For, Show } from 'solid-js';
import { A, useNavigate } from '@solidjs/router';
import { api } from '../api';
import { useAuth } from '../auth';
import type { Book, BookDetails, PageMeta, Paginated, Shelf } from '../types';

export default function Dashboard() {
  const auth = useAuth();
  const navigate = useNavigate();
  const [shelves] = createResource(() => api.get<Paginated<Shelf>>('/shelves?sort=name'));
  const [books, { refetch: refetchBooks }] = createResource(() =>
    api.get<Paginated<Book>>('/books?sort=name'),
  );
  const [recent] = createResource(() =>
    api.get<Paginated<PageMeta>>('/pages?sort=updated_at&order=desc&count=8'),
  );

  const [showNewBook, setShowNewBook] = createSignal(false);
  const [name, setName] = createSignal('');
  const [description, setDescription] = createSignal('');

  const createBook = async (e: Event) => {
    e.preventDefault();
    const created = await api.post<BookDetails>('/books', {
      name: name(),
      description: description(),
    });
    setShowNewBook(false);
    setName('');
    setDescription('');
    await refetchBooks();
    navigate(`/book/${created.slug}`);
  };

  return (
    <div class="dashboard">
      <section>
        <div class="section-head">
          <h2>Shelves</h2>
        </div>
        <div class="card-grid">
          <For each={shelves()?.data} fallback={<div class="empty-note">No shelves yet.</div>}>
            {shelf => (
              <A href={`/shelf/${shelf.slug}`} class="card shelf-card">
                <div class="card-icon">🗄</div>
                <div>
                  <div class="card-title">{shelf.name}</div>
                  <div class="card-sub">{shelf.description || 'Shelf'}</div>
                </div>
              </A>
            )}
          </For>
        </div>
      </section>

      <section>
        <div class="section-head">
          <h2>Books</h2>
          <Show when={auth.canEdit()}>
            <button class="btn btn-primary" onClick={() => setShowNewBook(!showNewBook())}>
              + New Book
            </button>
          </Show>
        </div>
        <Show when={showNewBook()}>
          <form class="inline-form" onSubmit={createBook}>
            <input
              placeholder="Book name"
              value={name()}
              onInput={e => setName(e.currentTarget.value)}
              required
            />
            <input
              placeholder="Description (optional)"
              value={description()}
              onInput={e => setDescription(e.currentTarget.value)}
            />
            <button class="btn btn-primary" type="submit">
              Create
            </button>
          </form>
        </Show>
        <div class="card-grid">
          <For each={books()?.data} fallback={<div class="empty-note">No books yet — create one.</div>}>
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
      </section>

      <section>
        <div class="section-head">
          <h2>Recently updated pages</h2>
        </div>
        <ul class="recent-list">
          <For each={recent()?.data} fallback={<div class="empty-note">Nothing yet.</div>}>
            {page => <RecentPage page={page} />}
          </For>
        </ul>
      </section>
    </div>
  );
}

function RecentPage(props: { page: PageMeta }) {
  const [book] = createResource(() => api.get<BookDetails>(`/books/${props.page.book_id}`));
  return (
    <li>
      <Show when={book()}>
        {b => (
          <A href={`/book/${b().slug}/page/${props.page.slug}`} class="recent-item">
            <span class="recent-icon">📄</span>
            <span class="recent-name">{props.page.name}</span>
            <span class="recent-book">in {b().name}</span>
            <span class="recent-time">{new Date(props.page.updated_at).toLocaleString()}</span>
          </A>
        )}
      </Show>
    </li>
  );
}
