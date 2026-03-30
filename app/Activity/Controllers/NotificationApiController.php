<?php

declare(strict_types=1);

namespace BookStack\Activity\Controllers;

use BookStack\Activity\Models\UserNotification;
use BookStack\Activity\Notifications\InAppNotificationService;
use BookStack\Http\ApiController;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Http\Response;

class NotificationApiController extends ApiController
{
    public function __construct(
        protected InAppNotificationService $service,
    ) {
    }

    /**
     * Get a paginated listing of notifications for the current user.
     * Supports 'count' (default 20, max 100), 'offset' (default 0), and
     * 'filter' ('all' or 'unread', default 'all') query parameters.
     */
    public function list(Request $request): JsonResponse
    {
        $this->preventGuestAccess();

        $count = max(1, min(100, (int) $request->get('count', 20)));
        $offset = max(0, (int) $request->get('offset', 0));
        $filter = $request->get('filter', 'all');

        $query = UserNotification::query()
            ->forUser(user()->id)
            ->orderBy('created_at', 'desc');

        if ($filter === 'unread') {
            $query->unread();
        }

        $total = $query->count();
        $notifications = $query->skip($offset)->take($count)->get();

        return response()->json([
            'data'  => $notifications->map(fn(UserNotification $n) => $this->notificationToArray($n))->values()->all(),
            'total' => $total,
        ]);
    }

    /**
     * Get the count of unread notifications for the current user.
     */
    public function unreadCount(): JsonResponse
    {
        $this->preventGuestAccess();

        return response()->json([
            'count' => $this->service->unreadCount(user()),
        ]);
    }

    /**
     * Mark a single notification as read.
     * The notification must belong to the current user.
     */
    public function markAsRead(string $id): JsonResponse
    {
        $this->preventGuestAccess();

        $notification = UserNotification::query()
            ->forUser(user()->id)
            ->find((int) $id);

        if ($notification === null) {
            return $this->jsonError('Notification not found', 404);
        }

        $this->service->markAsRead($notification);

        return response()->json($this->notificationToArray($notification));
    }

    /**
     * Mark all notifications as read for the current user.
     */
    public function markAllAsRead(): JsonResponse
    {
        $this->preventGuestAccess();

        $this->service->markAllAsRead(user());

        return response()->json(['status' => 'ok']);
    }

    /**
     * Delete a single notification.
     * The notification must belong to the current user.
     */
    public function destroy(string $id): Response|JsonResponse
    {
        $this->preventGuestAccess();

        $notification = UserNotification::query()
            ->forUser(user()->id)
            ->find((int) $id);

        if ($notification === null) {
            return $this->jsonError('Notification not found', 404);
        }

        $notification->delete();

        return response('', 204);
    }

    /**
     * Convert a notification model to the array shape used in API responses.
     */
    protected function notificationToArray(UserNotification $notification): array
    {
        return [
            'id'         => $notification->id,
            'type'       => $notification->type,
            'data'       => $notification->data,
            'read'       => !is_null($notification->read_at),
            'created_at' => $notification->created_at?->toIso8601String(),
        ];
    }
}
