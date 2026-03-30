<?php

namespace BookStack\Activity\Notifications;

use BookStack\Activity\Models\UserNotification;
use BookStack\Users\Models\User;
use Illuminate\Support\Collection;

class InAppNotificationService
{
    /**
     * Create a single in-app notification for a user.
     */
    public function notify(User $user, string $type, array $data): void
    {
        UserNotification::query()->create([
            'user_id'    => $user->id,
            'type'       => $type,
            'data'       => $data,
            'read_at'    => null,
            'created_at' => now(),
        ]);
    }

    /**
     * Create in-app notifications for many users at once.
     *
     * @param Collection<User> $users
     */
    public function notifyMany(Collection $users, string $type, array $data): void
    {
        $now = now();

        $rows = $users->map(fn(User $user) => [
            'user_id'    => $user->id,
            'type'       => $type,
            'data'       => json_encode($data),
            'read_at'    => null,
            'created_at' => $now,
        ])->all();

        if (!empty($rows)) {
            UserNotification::query()->insert($rows);
        }
    }

    /**
     * Get paginated notifications for a user, newest first.
     */
    public function getForUser(User $user, int $limit = 20, int $offset = 0): Collection
    {
        return UserNotification::query()
            ->forUser($user->id)
            ->orderBy('created_at', 'desc')
            ->skip($offset)
            ->take($limit)
            ->get();
    }

    /**
     * Get the count of unread notifications for a user.
     */
    public function unreadCount(User $user): int
    {
        return UserNotification::query()
            ->forUser($user->id)
            ->unread()
            ->count();
    }

    /**
     * Mark a single notification as read.
     */
    public function markAsRead(UserNotification $notification): void
    {
        $notification->markAsRead();
    }

    /**
     * Mark all notifications for a user as read.
     */
    public function markAllAsRead(User $user): void
    {
        UserNotification::query()
            ->forUser($user->id)
            ->unread()
            ->update(['read_at' => now()]);
    }

    /**
     * Delete notifications older than the given number of days.
     * Returns the number of deleted rows.
     */
    public function cleanup(int $daysOld = 90): int
    {
        return UserNotification::query()
            ->where('created_at', '<', now()->subDays($daysOld))
            ->delete();
    }
}
