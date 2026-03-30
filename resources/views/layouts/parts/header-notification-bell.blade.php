<div class="notification-bell-wrapper" x-data="notificationBell()" x-cloak>
    <div class="relative">
        <button
            type="button"
            class="notification-bell py-s"
            @click="open = !open"
            :aria-expanded="open ? 'true' : 'false'"
            aria-label="{{ trans('preferences.notifications') }}"
            title="{{ trans('preferences.notifications') }}">
            @icon('notifications')
            <span
                class="notification-badge"
                x-show="unreadCount > 0"
                x-text="unreadCount > 99 ? '99+' : unreadCount"
                aria-live="polite">
            </span>
        </button>

        <div
            class="notification-dropdown"
            x-show="open"
            @click.outside="open = false">

            <div class="notification-dropdown-header">
                <strong>{{ trans('preferences.notifications') }}</strong>
                <button
                    type="button"
                    class="text-button text-small"
                    @click="markAllAsRead()"
                    x-show="unreadCount > 0">
                    Mark all read
                </button>
            </div>

            <div x-show="loading" class="notification-dropdown-loading px-m py-s text-muted text-small">
                Loading...
            </div>

            <div x-show="!loading && notifications.length === 0" class="px-m py-s text-muted text-small italic">
                No notifications
            </div>

            <template x-for="notification in notifications" :key="notification.id">
                <div
                    class="notification-item"
                    :class="{ 'unread': !notification.read }"
                    @click="clickNotification(notification)"
                    role="button"
                    tabindex="0"
                    @keydown.enter="clickNotification(notification)">
                    <div class="notification-item-content">
                        <div class="notification-item-title" x-text="notification.data.title || ''"></div>
                        <div class="notification-item-message text-small text-muted" x-text="notification.data.message || ''"></div>
                        <div class="notification-item-time text-small text-muted" x-text="relativeTime(notification.created_at)"></div>
                    </div>
                </div>
            </template>

            <div class="notification-dropdown-footer">
                <a href="{{ url('/notifications') }}" class="text-small">View all notifications</a>
            </div>
        </div>
    </div>
</div>
