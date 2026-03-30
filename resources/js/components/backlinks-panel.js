/**
 * Alpine.js component data factory for the backlinks sidebar panel.
 * Lazy-loads linked and unlinked mentions for the current entity via AJAX.
 *
 * @param {Object} opts
 * @param {string} opts.entityType  - morph class string, e.g. 'page', 'chapter', 'book'
 * @param {number} opts.entityId
 * @returns {Object}
 */
export function backlinksPanel({entityType, entityId}) {
    return {
        open: false,
        loading: false,
        loaded: false,
        linked: [],
        unlinked: [],

        get totalCount() {
            return this.linked.length + this.unlinked.length;
        },

        init() {
            // Lazy load: fetch when panel is first opened
            this.$watch('open', (isOpen) => {
                if (isOpen && !this.loaded) {
                    this.load();
                }
            });
        },

        async load() {
            this.loading = true;
            try {
                const resp = await window.$http.get(`/ajax/backlinks/${entityType}/${entityId}`);
                this.linked = resp.data.linked || [];
                this.unlinked = resp.data.unlinked || [];
                this.loaded = true;
            } catch (err) {
                this.linked = [];
                this.unlinked = [];
            } finally {
                this.loading = false;
            }
        },

        typeLabel(type) {
            const labels = {
                page: 'Page',
                chapter: 'Chapter',
                book: 'Book',
                bookshelf: 'Shelf',
            };
            return labels[type] || type;
        },
    };
}
