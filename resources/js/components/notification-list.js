/**
 * Alpine.js component data factory for the full notifications page.
 * Handles paginated listing, filtering, marking as read, and deletion.
 *
 * @param {Object} opts
 * @param {string} opts.filter - 'all' or 'unread'
 * @returns {Object}
 */
export function notificationList({ filter }) {
    return {
        filter,
        loading: false,
        notifications: window.__initialNotifications || [],

        async markAsRead(notification) {
            if (notification.read) return;
            try {
                await window.$http.put(`/ajax/notifications/${notification.id}/read`);
                notification.read = true;
            } catch (err) {
                // Ignore
            }
        },

        async markAllAsRead() {
            try {
                await window.$http.post('/ajax/notifications/mark-all-read');
                this.notifications.forEach(n => { n.read = true; });
            } catch (err) {
                // Ignore
            }
        },

        async deleteNotification(notification) {
            try {
                await window.$http.delete(`/ajax/notifications/${notification.id}`);
                this.notifications = this.notifications.filter(n => n.id !== notification.id);
            } catch (err) {
                // Ignore
            }
        },

        async clickAndNavigate(notification) {
            await this.markAsRead(notification);
            if (notification.data && notification.data.link) {
                window.location.href = notification.data.link;
            }
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
