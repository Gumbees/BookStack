/**
 * collaborative-editor.ts
 * Alpine.js component that wraps the WYSIWYG editor with real-time collaboration.
 *
 * When collab is enabled this component:
 *  1. Fetches a short-lived JWT from BookStack's /collab/token/{pageId} endpoint.
 *  2. Loads the WASM collab module (built by wasm-pack).
 *  3. Creates a CollabSession connecting to the Rust sidecar over WebSocket.
 *  4. Activates the collab-bridge to wire Lexical <-> Yrs CRDT bidirectionally.
 *  5. Exposes peer presence state for the collab-presence component to render.
 */

import type { CollabPeer } from '../wysiwyg/services/collab-bridge';
import { activateCollabBridge } from '../wysiwyg/services/collab-bridge';

interface CollabEditorOpts {
    pageId: number;
    enabled: boolean;
}

interface CollabEditorState {
    connected: boolean;
    peers: CollabPeer[];
    _session: any;
    _bridgeTeardown: (() => void) | null;
    _presenceEl: any;

    init(): Promise<void>;
    disconnect(): void;
    destroy(): void;
}

export function collaborativeEditor({ pageId, enabled }: CollabEditorOpts): CollabEditorState {
    return {
        connected: false,
        peers: [] as CollabPeer[],
        _session: null,
        _bridgeTeardown: null,
        _presenceEl: null,

        async init() {
            if (!enabled || !pageId) {
                return;
            }

            // Wait for the Lexical editor to be fully initialised.
            await waitForEditor();

            const lexicalEditor = getLexicalEditor();
            if (!lexicalEditor) {
                console.warn('[collab] Lexical editor not found, collab disabled.');
                return;
            }

            // Fetch the collab token from BookStack.
            let wsUrl: string;
            let token: string;
            try {
                const resp = await window.$http.get(`/collab/token/${pageId}`);
                wsUrl = resp.data.ws_url;
                token = resp.data.token;
            } catch (err) {
                console.warn('[collab] Failed to fetch collab token:', err);
                return;
            }

            // Dynamically import the WASM module built by wasm-pack.
            let wasm: any;
            try {
                wasm = await import(/* webpackIgnore: true */ '/dist/collab-wasm/collab_wasm.js');
                await wasm.default();
            } catch (err) {
                console.warn('[collab] Failed to load WASM module:', err);
                return;
            }

            // Create the CRDT session.
            try {
                this._session = new wasm.CollabSession(pageId, wsUrl, token);
            } catch (err) {
                console.warn('[collab] Failed to create CollabSession:', err);
                return;
            }

            this.connected = true;

            // Wire the bridge.
            this._bridgeTeardown = activateCollabBridge({
                editor: lexicalEditor,
                session: this._session,
                onPeersChanged: (peers: CollabPeer[]) => {
                    this.peers = peers;
                    // Forward to the presence component if available.
                    if (this._presenceEl && typeof this._presenceEl.setPeers === 'function') {
                        this._presenceEl.setPeers(peers);
                    }
                },
            });
        },

        disconnect() {
            if (this._bridgeTeardown) {
                this._bridgeTeardown();
                this._bridgeTeardown = null;
            }
            if (this._session) {
                try {
                    this._session.disconnect();
                } catch {
                    // Ignore errors on disconnect.
                }
                this._session = null;
            }
            this.connected = false;
            this.peers = [];
        },

        destroy() {
            this.disconnect();
        },
    };
}

/** Wait for the Lexical editor to emit its post-init event. */
function waitForEditor(): Promise<void> {
    return new Promise(resolve => {
        // Check if already ready.
        if (window.wysiwyg) {
            resolve();
            return;
        }
        const handler = () => {
            window.removeEventListener('editor-wysiwyg::post-init', handler);
            resolve();
        };
        // The editor emits a custom DOM event on its container element.
        // We listen at the window level which bubbles up.
        window.addEventListener('editor-wysiwyg::post-init', handler);
        // Fallback: if the event never fires within 5 seconds, continue anyway.
        setTimeout(() => {
            window.removeEventListener('editor-wysiwyg::post-init', handler);
            resolve();
        }, 5000);
    });
}

/** Retrieve the Lexical editor instance from the global wysiwyg interface. */
function getLexicalEditor(): any {
    const iface = (window as any).wysiwyg;
    if (!iface) {
        return null;
    }
    // The SimpleWysiwygEditorInterface wraps the editor in context.
    // We reach into context.editor (the actual LexicalEditor).
    return iface?.['context']?.['editor'] ?? null;
}
