/**
 * Alpine.js component data factory for the collaborative scratch notes panel.
 *
 * @param {Object} opts
 * @param {number} opts.pageId
 * @param {boolean} opts.canEdit
 * @param {number} opts.currentUserId
 * @param {boolean} opts.isAdmin
 * @returns {Object}
 */
export function scratchNotes({pageId, canEdit, currentUserId, isAdmin}) {
    return {
        open: false,
        notes: [],
        newNote: '',
        loading: true,
        editingId: null,
        editContent: '',

        init() {
            this.$watch('open', val => {
                if (val && this.loading) {
                    this.load();
                }
            });
        },

        async load() {
            this.loading = true;
            try {
                const resp = await window.$http.get(`/ajax/page/${pageId}/scratch-notes`);
                this.notes = resp.data;
            } catch (err) {
                console.error('Failed to load scratch notes', err);
            } finally {
                this.loading = false;
            }
        },

        async addNote() {
            const content = this.newNote.trim();
            if (!content) return;
            try {
                const resp = await window.$http.post(`/ajax/page/${pageId}/scratch-notes`, {content});
                this.notes.push(resp.data);
                this.newNote = '';
            } catch (err) {
                console.error('Failed to add note', err);
            }
        },

        startEdit(note) {
            this.editingId = note.id;
            this.editContent = note.content;
        },

        async saveEdit(note) {
            const content = this.editContent.trim();
            if (!content) return;
            try {
                const resp = await window.$http.put(`/ajax/page/${pageId}/scratch-notes/${note.id}`, {content});
                note.content = resp.data.content;
                note.updated_at = resp.data.updated_at;
                this.editingId = null;
            } catch (err) {
                console.error('Failed to save note', err);
            }
        },

        cancelEdit() {
            this.editingId = null;
            this.editContent = '';
        },

        async deleteNote(note) {
            try {
                await window.$http.delete(`/ajax/page/${pageId}/scratch-notes/${note.id}`);
                this.notes = this.notes.filter(n => n.id !== note.id);
            } catch (err) {
                console.error('Failed to delete note', err);
            }
        },

        canModify(note) {
            return note.user_id === currentUserId || isAdmin;
        },

        formatTime(iso) {
            if (!iso) return '';
            const date = new Date(iso);
            const now = new Date();
            const diffMs = now - date;
            const diffMins = Math.floor(diffMs / 60000);
            if (diffMins < 1) return 'just now';
            if (diffMins < 60) return `${diffMins}m ago`;
            const diffHours = Math.floor(diffMins / 60);
            if (diffHours < 24) return `${diffHours}h ago`;
            const diffDays = Math.floor(diffHours / 24);
            if (diffDays < 7) return `${diffDays}d ago`;
            return date.toLocaleDateString();
        },
    };
}
