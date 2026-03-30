/**
 * collab-bridge.ts
 * Bridge between the Lexical WYSIWYG editor and the WASM CRDT session.
 *
 * v1 uses an HTML-string based approach: on each local Lexical change the
 * editor serialises to HTML and we push that as a text update to Yrs. On
 * receiving a remote update we replace the editor content with the new text.
 * A full node-level Yrs<->Lexical binding is a future enhancement.
 */

import type { LexicalEditor } from 'lexical';
import { getEditorContentAsHtml, setEditorContentFromHtml } from '../utils/actions';

export interface CollabPeer {
    user_id: number;
    user_name: string;
    color: string;
    cursor_pos: number;
    selection_start: number;
    selection_end: number;
}

export interface CollabBridgeOptions {
    /** The Lexical editor instance to bridge. */
    editor: LexicalEditor;
    /** The WASM CollabSession instance (typed as any to avoid WASM coupling at compile time). */
    session: any;
    /** Called when remote peer awareness state changes. */
    onPeersChanged?: (peers: CollabPeer[]) => void;
}

/**
 * Activate the bridge: wire the Lexical editor to the WASM session bidirectionally.
 * Returns a teardown function that removes all listeners.
 */
export function activateCollabBridge(opts: CollabBridgeOptions): () => void {
    const { editor, session, onPeersChanged } = opts;

    // Debounce flag to avoid re-entrancy when we apply a remote update to Lexical,
    // which would otherwise trigger another local-change event.
    let applyingRemote = false;
    let lastLocalHtml = '';

    // Remote update handler: called by WASM when the Yrs doc changes.
    const handleRemoteUpdate = (newText: string) => {
        if (!newText || newText === lastLocalHtml) {
            return;
        }
        applyingRemote = true;
        setEditorContentFromHtml(editor, newText);
        lastLocalHtml = newText;
        applyingRemote = false;
    };

    // Awareness handler: relay peer list to the component layer.
    const handleAwareness = (peers: CollabPeer[]) => {
        if (onPeersChanged) {
            onPeersChanged(peers);
        }
    };

    session.on_update(handleRemoteUpdate);
    session.on_awareness(handleAwareness);

    // Local change listener: serialise Lexical to HTML and push to Yrs as a text update.
    // We throttle to avoid flooding during fast typing.
    let throttleTimer: ReturnType<typeof setTimeout> | null = null;

    const teardownLocalListener = editor.registerUpdateListener(({ editorState }) => {
        if (applyingRemote) {
            return;
        }
        if (throttleTimer !== null) {
            clearTimeout(throttleTimer);
        }
        throttleTimer = setTimeout(async () => {
            const html = await getEditorContentAsHtml(editor);
            if (html !== lastLocalHtml) {
                lastLocalHtml = html;
                // Encode the HTML as UTF-8 bytes and push to the WASM session as an update.
                // The WASM collab-wasm exposes set_text() for this purpose.
                if (typeof session.set_text === 'function') {
                    session.set_text(html);
                }
            }
        }, 150);
    });

    // Selection change listener for awareness (cursor tracking).
    const teardownSelectionListener = editor.registerUpdateListener(({ editorState }) => {
        if (applyingRemote) {
            return;
        }
        editorState.read(() => {
            const selection = window.getSelection();
            if (!selection) {
                return;
            }
            const cursorPos = selection.focusOffset;
            const start = Math.min(selection.anchorOffset, selection.focusOffset);
            const end = Math.max(selection.anchorOffset, selection.focusOffset);
            session.set_awareness(cursorPos, start, end);
        });
    });

    return () => {
        teardownLocalListener();
        teardownSelectionListener();
        if (throttleTimer !== null) {
            clearTimeout(throttleTimer);
        }
    };
}
