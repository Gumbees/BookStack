/**
 * Alpine.js component data factory for the notification bell in the header.
 * Polls for unread count every 60 seconds and shows a dropdown with recent notifications.
 *
 * @returns {Object}
 */
export function notificationBell() {
    return {
        open: false,
        unreadCount: 0,
        notifications: [],
        loading: false,
        pollInterval: null,

        init() {
            this.fetchCount();
            this.pollInterval = setInterval(() => this.fetchCount(), 60000);

            this.$watch('open', (isOpen) => {
                if (isOpen) {
                    this.loadDropdown();
                }
            });
        },

        destroy() {
            if (this.pollInterval) {
                clearInterval(this.pollInterval);
            }
        },

        async fetchCount() {
            try {
                const resp = await window.$http.get('/ajax/notifications/unread-count');
                this.unreadCount = resp.data.count || 0;
            } catch (err) {
                // Fail silently — don't disrupt the page for a badge count
            }
        },

        async loadDropdown() {
            this.loading = true;
            try {
                const resp = await window.$http.get('/ajax/notifications', {
                    params: { limit: 5, unread: 'false' },
                });
                this.notifications = resp.data.notifications || [];
            } catch (err) {
                this.notifications = [];
            } finally {
                this.loading = false;
            }
        },

        async markAllAsRead() {
            try {
                await window.$http.post('/ajax/notifications/mark-all-read');
                this.unreadCount = 0;
                this.notifications.forEach(n => { n.read = true; });
            } catch (err) {
                // Ignore
            }
        },

        async clickNotification(notification) {
            if (!notification.read) {
                try {
                    await window.$http.put(`/ajax/notifications/${notification.id}/read`);
                    notification.read = true;
                    if (this.unreadCount > 0) this.unreadCount--;
                } catch (err) {
                    // Ignore
                }
            }

            if (notification.data && notification.data.link) {
                window.location.href = notification.data.link;
            }
        },

        typeIcon(type) {
            const icons = {
                page_create: '📄',
                page_update: '✏️',
                comment_create: '💬',
                mention: '@',
                scratch_note_mention: '@',
            };
            return icons[type] || '🔔';
        },

        relativeTime(isoString) {
            if (!isoString) return '';
            const date = new Date(isoString);
            const diffMs = Date.now() - date.getTime();
            const diffMins = Math.floor(diffMs / 60000);
            if (diffMins < 1) return 'just now';
            if (diffMins < 60) return `${diffMins}m ago`;
            const diffHours = Math.floor(diffMins / 60);
            if (diffHours < 24) return `${diffHours}h ago`;
            const diffDays = Math.floor(diffHours / 24);
            return diffDays < 7 ? `${diffDays}d ago` : date.toLocaleDateString();
        },
    };
}
