# Notification System

This fork adds an in-app notification system and a follow/watch system for tracking content changes.

## In-app notifications

When enabled, a bell icon appears in the top navigation bar. Clicking it opens the notification dropdown. The bell shows an unread count badge when new notifications are present.

The full notifications inbox is available at `/notifications`. It supports filtering to unread-only and paginated browsing.

### Notification types

| Type | When triggered |
|------|---------------|
| `page_creation` | A new page is created in a watched book/chapter |
| `page_update` | A page you follow is updated |
| `comment_creation` | A comment is added to a page you follow |
| `comment_mention` | You are @mentioned in a comment |
| `scratch_note_mention` | You are @mentioned in a scratch note |

### User preferences

Users configure their own preferences at `/my-account/notifications`:

- Notify on own page changes
- Notify on own page comments
- Notify on comment replies
- Notify on comment mentions

These are per-user settings. Admin defaults are configurable at `/settings/notifications`.

## Follow / watch system

Users can follow (watch) any entity: shelf, book, chapter, or page. Following triggers notifications when content in that entity changes.

### Watch levels

| Level | Value | Description |
|-------|-------|-------------|
| `default` | `-1` | No explicit preference set; inherits from parent |
| `ignore` | `0` | Suppress all notifications for this entity |
| `new` | `1` | Notify when new child content is created |
| `updates` | `2` | Notify on new content and updates |
| `comments` | `3` | Notify on new content, updates, and comments |

Pages don't support the `new` level since they have no children.

### Hierarchy

Watch settings cascade. If a book is set to `updates` and a child page has no explicit setting, the page inherits `updates`. An explicit `ignore` on a page overrides a parent's watch level.

A watch button appears on every entity's page for authenticated users who have the `receive-notifications` permission.

## @mentions

Type `@Name` in a comment or scratch note to mention another user. The system matches the text against user display names (case-sensitive exact match on the full name). The mentioned user receives a notification if:

- They have the `receive-notifications` permission
- They have view access to the containing page
- They have not already been notified for that mention (update deduplication)

Mentions in comments go through `CommentMentionNotificationHandler`. Mentions in scratch notes are handled directly by the scratch note controllers.

Both channels can be disabled globally by an admin.

## Admin settings

Located at `/settings/notifications`. All settings are stored in the BookStack settings table (not `.env`).

| Setting key | Default | Description |
|-------------|---------|-------------|
| `notifications.in_app_enabled` | `true` | Show the notification bell and deliver in-app notifications |
| `notifications.email_enabled` | `true` | Send email notification messages |
| `notifications.mention_enabled` | `true` | Process @mentions and deliver mention notifications |
| `notifications.default_own_page_changes` | `true` | Default user preference: notify on own page changes |
| `notifications.default_own_page_comments` | `true` | Default user preference: notify on own page comments |
| `notifications.default_comment_replies` | `true` | Default user preference: notify on comment replies |
| `notifications.default_comment_mentions` | `true` | Default user preference: notify on comment @mentions |
| `notifications.cleanup_days` | `90` | Delete in-app notifications older than this many days |

## Notification cleanup

Old notifications are pruned by an Artisan command:

```bash
php artisan bookstack:cleanup-notifications
```

The retention period is read from the `notifications.cleanup_days` setting (default 90 days). Schedule this command to run daily via cron or Laravel's task scheduler to prevent unbounded growth:

```php
// In App\Console\Kernel or routes/console.php
$schedule->command('bookstack:cleanup-notifications')->daily();
```

## API endpoints

All notification and follow endpoints require an authenticated user (API token).

### Notifications

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `api/notifications` | Paginated list. Params: `count` (max 100), `offset`, `filter` (`all`/`unread`) |
| `GET` | `api/notifications/unread-count` | Returns `{"count": N}` |
| `PUT` | `api/notifications/{id}/read` | Mark one notification as read |
| `POST` | `api/notifications/mark-all-read` | Mark all notifications as read |
| `DELETE` | `api/notifications/{id}` | Delete a notification |

Example notification object:

```json
{
  "id": 42,
  "type": "page_update",
  "data": {
    "title": "My Page",
    "message": "The page has been updated.",
    "link": "https://example.com/books/my-book/page/my-page"
  },
  "read": false,
  "created_at": "2026-03-15T14:30:00+00:00"
}
```

### Follows

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `api/follows` | List all current user's follows. Params: `count`, `offset` |
| `GET` | `api/follows/{type}/{id}` | Get follow status for a specific entity |
| `PUT` | `api/follows/{type}/{id}` | Set or update follow level. Body: `{"level": 1}` |
| `DELETE` | `api/follows/{type}/{id}` | Unfollow an entity |

Valid `type` values: `shelf`, `book`, `chapter`, `page`.

Level values: `0` (ignore), `1` (new), `2` (updates), `3` (comments).

Example response for `GET api/follows/book/5`:

```json
{
  "following": true,
  "level": 2,
  "level_label": "updates"
}
```
