/**
 * Alpine.js component for rendering connected collaborator presence indicators.
 * Shows a row of colored avatar circles in the toolbar area with name tooltips.
 * Also injects colored cursor carets into the editor for each remote peer.
 */

import type { CollabPeer } from '../wysiwyg/services/collab-bridge';

interface CollabPresenceState {
    peers: CollabPeer[];
    setPeers(peers: CollabPeer[]): void;
    _cursors: Map<number, HTMLElement>;
    _updateCursorDecorations(peers: CollabPeer[]): void;
    destroy(): void;
}

export function collabPresence(): CollabPresenceState {
    return {
        peers: [] as CollabPeer[],
        _cursors: new Map<number, HTMLElement>(),

        setPeers(peers: CollabPeer[]) {
            this.peers = peers;
            this._updateCursorDecorations(peers);
        },

        _updateCursorDecorations(peers: CollabPeer[]) {
            const editorContent = document.querySelector('.page-content') as HTMLElement | null;
            if (!editorContent) {
                return;
            }

            const seen = new Set<number>();

            for (const peer of peers) {
                seen.add(peer.user_id);

                let caret = this._cursors.get(peer.user_id);
                if (!caret) {
                    caret = document.createElement('span');
                    caret.className = 'collab-cursor';
                    caret.setAttribute('aria-hidden', 'true');
                    caret.style.setProperty('--collab-cursor-color', peer.color);

                    const label = document.createElement('span');
                    label.className = 'collab-cursor-label';
                    label.textContent = peer.user_name;
                    caret.appendChild(label);

                    editorContent.appendChild(caret);
                    this._cursors.set(peer.user_id, caret);
                }

                // Position the caret based on cursor_pos (character offset).
                // For the v1 HTML bridge this is approximate; a full binding
                // would map Yrs relative positions to DOM ranges.
                caret.style.display = 'block';
                caret.dataset.cursorPos = String(peer.cursor_pos);
            }

            // Remove carets for peers who have left.
            for (const [userId, el] of this._cursors) {
                if (!seen.has(userId)) {
                    el.remove();
                    this._cursors.delete(userId);
                }
            }
        },

        destroy() {
            for (const el of this._cursors.values()) {
                el.remove();
            }
            this._cursors.clear();
        },
    };
}
