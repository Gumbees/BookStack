/**
 * Alpine.js component data factory for the scratch notes panel.
 * Loaded once per page view when the user is authenticated.
 *
 * @param {Object} opts
 * @param {number} opts.pageId
 * @returns {Object}
 */
export function scratchNotes({pageId}) {
    return {
        open: false,
        content: '',
        statusText: '',
        loading: true,

        init() {
            this.load();
        },

        async load() {
            this.loading = true;
            this.statusText = '';
            try {
                const resp = await window.$http.get(`/ajax/page/${pageId}/scratch-note`);
                this.content = resp.data.content || '';
                if (resp.data.updated_at) {
                    this.statusText = this.formatSavedTime(resp.data.updated_at);
                }
            } catch (err) {
                this.statusText = 'Failed to load';
            } finally {
                this.loading = false;
            }
        },

        async save() {
            this.statusText = 'Saving...';
            try {
                const resp = await window.$http.put(`/ajax/page/${pageId}/scratch-note`, {
                    content: this.content,
                });
                this.statusText = this.formatSavedTime(resp.data.updated_at);
            } catch (err) {
                this.statusText = 'Failed to save';
            }
        },

        formatSavedTime(isoString) {
            if (!isoString) return 'Saved';
            const date = new Date(isoString);
            const hours = date.getHours();
            const minutes = String(date.getMinutes()).padStart(2, '0');
            const ampm = hours >= 12 ? 'PM' : 'AM';
            const h = hours % 12 || 12;
            return `Saved at ${h}:${minutes} ${ampm}`;
        },
    };
}
