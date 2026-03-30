<?php

namespace BookStack\Activity\Controllers;

use BookStack\Activity\Models\UserNotification;
use BookStack\Activity\Notifications\InAppNotificationService;
use BookStack\Http\Controller;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;

class NotificationController extends Controller
{
    public function __construct(
        protected InAppNotificationService $service,
    ) {
    }

    /**
     * Show the full notifications inbox page.
     */
    public function index(Request $request)
    {
        $this->preventGuestAccess();

        $filter = $request->get('filter', 'all');
        $page = max(1, intval($request->get('page', 1)));
        $perPage = 20;
        $offset = ($page - 1) * $perPage;

        $query = UserNotification::query()
            ->forUser(user()->id)
            ->orderBy('created_at', 'desc');

        if ($filter === 'unread') {
            $query->unread();
        }

        $total = $query->count();
        $notifications = $query->skip($offset)->take($perPage)->get();

        $totalPages = (int) ceil($total / $perPage);

        $this->setPageTitle(trans('preferences.notifications'));

        return view('notifications.index', [
            'notifications' => $notifications,
            'filter'        => $filter,
            'currentPage'   => $page,
            'totalPages'    => $totalPages,
            'total'         => $total,
            'unreadCount'   => $this->service->unreadCount(user()),
        ]);
    }

    /**
     * Return a paginated JSON list of notifications.
     */
    public function list(Request $request): JsonResponse
    {
        $this->preventGuestAccess();

        $limit = min(50, max(1, intval($request->get('limit', 20))));
        $offset = max(0, intval($request->get('offset', 0)));
        $unreadOnly = $request->get('unread') === 'true';

        $query = UserNotification::query()
            ->forUser(user()->id)
            ->orderBy('created_at', 'desc');

        if ($unreadOnly) {
            $query->unread();
        }

        $items = $query->skip($offset)->take($limit)->get();

        return response()->json([
            'notifications' => $items->map(fn(UserNotification $n) => $this->notificationToArray($n)),
            'offset'        => $offset,
            'limit'         => $limit,
        ]);
    }

    /**
     * Return the unread notification count.
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
     */
    public function markAsRead(int $id): JsonResponse
    {
        $this->preventGuestAccess();

        $notification = UserNotification::query()
            ->forUser(user()->id)
            ->findOrFail($id);

        $this->service->markAsRead($notification);

        return response()->json(['status' => 'ok']);
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
     */
    public function destroy(int $id): JsonResponse
    {
        $this->preventGuestAccess();

        $notification = UserNotification::query()
            ->forUser(user()->id)
            ->findOrFail($id);

        $notification->delete();

        return response()->json([], 204);
    }

    /**
     * Convert a notification model to the array shape used in responses.
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
