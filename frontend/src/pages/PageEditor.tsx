import { createEffect, createResource, createSignal, onCleanup, For, Show } from 'solid-js';
import { A, useNavigate, useParams } from '@solidjs/router';
import * as Y from 'yjs';
import { WebsocketProvider } from 'y-websocket';
import { yCollab, yUndoManagerKeymap } from 'y-codemirror.next';
import { EditorState } from '@codemirror/state';
import { EditorView, keymap, lineNumbers } from '@codemirror/view';
import { defaultKeymap, indentWithTab } from '@codemirror/commands';
import { markdown } from '@codemirror/lang-markdown';
import { defaultHighlightStyle, syntaxHighlighting } from '@codemirror/language';
import { api, collabServerBase, getToken, userColor } from '../api';
import { useAuth } from '../auth';
import type { Page } from '../types';

interface Peer {
  name: string;
  color: string;
}

/// Collaborative markdown editor: a CodeMirror 6 instance bound to a shared
/// Yjs document synced through the Rust collab engine. Every connected editor
/// sees keystrokes and cursors live; the server flattens the CRDT back to
/// markdown/HTML in Postgres.
export default function PageEditor() {
  const params = useParams();
  const auth = useAuth();
  const navigate = useNavigate();
  const [page] = createResource(
    () => `${params.bookSlug}/${params.pageSlug}`,
    () => api.get<Page>(`/pages/by-slugs/${params.bookSlug}/${params.pageSlug}`),
  );
  const [peers, setPeers] = createSignal<Peer[]>([]);
  const [connection, setConnection] = createSignal<'connecting' | 'connected' | 'disconnected'>('connecting');
  const [saveState, setSaveState] = createSignal<'idle' | 'saving' | 'saved'>('idle');
  // Bumped when the server closes the room with code 4409 (content replaced
  // out-of-band): the whole doc + provider + editor must be rebuilt fresh —
  // resyncing the old Y.Doc would merge stale state back in.
  const [generation, setGeneration] = createSignal(0);
  let editorHost: HTMLDivElement | undefined;

  createEffect(() => {
    generation();
    const p = page();
    const user = auth.user();
    if (!p || !user || !editorHost) return;

    const doc = new Y.Doc();
    const provider = new WebsocketProvider(collabServerBase(), String(p.id), doc, {
      params: { token: getToken() ?? '' },
    });
    // The server binds page text to this named root (TEXT_ROOT in Rust).
    const ytext = doc.getText('content');
    const undoManager = new Y.UndoManager(ytext);

    provider.awareness.setLocalStateField('user', {
      name: user.name,
      color: userColor(user.id),
      colorLight: userColor(user.id) + '33',
    });
    provider.on('status', (event: { status: string }) => {
      setConnection(event.status === 'connected' ? 'connected' : 'connecting');
    });
    provider.on('connection-close', (event: CloseEvent | null) => {
      setConnection('disconnected');
      if (event && event.code === 4409) {
        // Server replaced the page content (REST/MCP edit). Stop reconnect
        // attempts with the stale doc and rebuild after the server's
        // invalidation window.
        provider.disconnect();
        setTimeout(() => setGeneration(g => g + 1), 1800);
      }
    });

    const refreshPeers = () => {
      const states = [...provider.awareness.getStates().entries()];
      setPeers(
        states
          .filter(([clientId]) => clientId !== provider.awareness.clientID)
          .map(([, state]) => (state as { user?: Peer }).user)
          .filter((u): u is Peer => !!u),
      );
    };
    provider.awareness.on('change', refreshPeers);

    const view = new EditorView({
      state: EditorState.create({
        doc: '',
        extensions: [
          keymap.of([...yUndoManagerKeymap, ...defaultKeymap, indentWithTab]),
          lineNumbers(),
          EditorView.lineWrapping,
          markdown(),
          syntaxHighlighting(defaultHighlightStyle),
          yCollab(ytext, provider.awareness, { undoManager }),
        ],
      }),
      parent: editorHost,
    });

    onCleanup(() => {
      provider.awareness.off('change', refreshPeers);
      view.destroy();
      provider.destroy();
      doc.destroy();
    });
  });

  const saveNow = async () => {
    const p = page();
    if (!p) return;
    setSaveState('saving');
    try {
      await api.post(`/pages/${p.id}/collab/save`);
      setSaveState('saved');
      setTimeout(() => setSaveState('idle'), 2000);
    } catch {
      setSaveState('idle');
    }
  };

  const finish = async () => {
    await saveNow();
    navigate(`/book/${params.bookSlug}/page/${params.pageSlug}`);
  };

  return (
    <div class="editor-wrap">
      <Show when={page()} fallback={<div class="empty-note">Loading…</div>}>
        {p => (
          <>
            <div class="editor-toolbar">
              <nav class="crumbs">
                <A href={`/book/${params.bookSlug}`}>{params.bookSlug}</A> <span>/</span>{' '}
                <strong>{p().name}</strong>
              </nav>
              <div class="editor-status">
                <span class={`conn-dot conn-${connection()}`} />
                <span class="conn-label">{connection()}</span>
                <div class="peer-chips">
                  <For each={peers()}>
                    {peer => (
                      <span class="peer-chip" style={{ 'background-color': peer.color }} title={peer.name}>
                        {peer.name.slice(0, 1).toUpperCase()}
                      </span>
                    )}
                  </For>
                  <Show when={peers().length > 0}>
                    <span class="peer-count">
                      {peers().length} other{peers().length === 1 ? '' : 's'} editing
                    </span>
                  </Show>
                </div>
              </div>
              <div class="btn-row">
                <button class="btn" onClick={saveNow} disabled={saveState() === 'saving'}>
                  {saveState() === 'saving' ? 'Saving…' : saveState() === 'saved' ? 'Saved ✓' : 'Save'}
                </button>
                <button class="btn btn-primary" onClick={finish}>
                  Done
                </button>
              </div>
            </div>
            <div class="editor-note">
              Markdown · changes sync live to everyone in this page and auto-persist every few seconds.
            </div>
          </>
        )}
      </Show>
      <div class="editor-host" ref={editorHost} />
    </div>
  );
}
