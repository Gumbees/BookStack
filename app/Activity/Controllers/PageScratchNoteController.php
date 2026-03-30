<?php

namespace BookStack\Activity\Controllers;

use BookStack\Activity\Models\PageScratchNote;
use BookStack\Activity\Models\ScratchNoteMentionHistory;
use BookStack\Activity\Notifications\InAppNotificationService;
use BookStack\Entities\Queries\PageQueries;
use BookStack\Http\Controller;
use BookStack\Permissions\Permission;
use BookStack\Permissions\PermissionApplicator;
use BookStack\Settings\UserNotificationPreferences;
use BookStack\Users\Models\User;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Log;

class PageScratchNoteController extends Controller
{
    public function __construct(
        protected PageQueries $pageQueries,
    ) {
    }

    /**
     * List all scratch notes for a page.
     * Requires page view permission.
     */
    public function index(int $pageId): JsonResponse
    {
        $this->preventGuestAccess();

        $page = $this->pageQueries->findVisibleById($pageId);
        if ($page === null) {
            return $this->jsonError('Page not found', 404);
        }

        $notes = PageScratchNote::query()
            ->where('page_id', $pageId)
            ->with('user:id,name')
            ->orderBy('created_at', 'asc')
            ->get()
            ->map(fn (PageScratchNote $note) => $this->noteToArray($note));

        return response()->json($notes);
    }

    /**
     * Add a new scratch note to a page.
     * Requires page edit permission.
     */
    public function store(Request $request, int $pageId): JsonResponse
    {
        $this->preventGuestAccess();

        $page = $this->pageQueries->findVisibleById($pageId);
        if ($page === null) {
            return $this->jsonError('Page not found', 404);
        }

        $this->checkOwnablePermission(Permission::PageUpdate, $page);

        $input = $this->validate($request, [
            'content' => ['required', 'string', 'max:2000'],
        ]);

        $note = PageScratchNote::query()->create([
            'page_id' => $pageId,
            'user_id' => user()->id,
            'content' => $input['content'],
        ]);

        $note->load('user:id,name');

        // Process @mentions on creation
        if (setting('notifications.mention_enabled', true) && setting('notifications.in_app_enabled', true)) {
            $this->processMentions($note, $input['content'], isUpdate: false);
        }

        return response()->json($this->noteToArray($note), 201);
    }

    /**
     * Update an existing scratch note.
     * Requires page edit permission and note authorship (or admin delete permission).
     */
    public function update(Request $request, int $pageId, int $noteId): JsonResponse
    {
        $this->preventGuestAccess();

        $page = $this->pageQueries->findVisibleById($pageId);
        if ($page === null) {
            return $this->jsonError('Page not found', 404);
        }

        $this->checkOwnablePermission(Permission::PageUpdate, $page);

        $note = PageScratchNote::query()
            ->where('id', $noteId)
            ->where('page_id', $pageId)
            ->first();

        if ($note === null) {
            return $this->jsonError('Note not found', 404);
        }

        if ($note->user_id !== user()->id && !user()->hasSystemRole('admin')) {
            return $this->jsonError(trans('errors.permissionJson'), 403);
        }

        $input = $this->validate($request, [
            'content' => ['required', 'string', 'max:2000'],
        ]);

        $note->content = $input['content'];
        $note->save();

        $note->load('user:id,name');

        // Process @mentions on update (skip previously notified users)
        if (setting('notifications.mention_enabled', true) && setting('notifications.in_app_enabled', true)) {
            $this->processMentions($note, $input['content'], isUpdate: true);
        }

        return response()->json($this->noteToArray($note));
    }

    /**
     * Delete a scratch note.
     * Requires page edit permission and note authorship (or admin delete permission).
     */
    public function destroy(int $pageId, int $noteId): JsonResponse
    {
        $this->preventGuestAccess();

        $page = $this->pageQueries->findVisibleById($pageId);
        if ($page === null) {
            return $this->jsonError('Page not found', 404);
        }

        $this->checkOwnablePermission(Permission::PageUpdate, $page);

        $note = PageScratchNote::query()
            ->where('id', $noteId)
            ->where('page_id', $pageId)
            ->first();

        if ($note === null) {
            return $this->jsonError('Note not found', 404);
        }

        if ($note->user_id !== user()->id && !user()->hasSystemRole('admin')) {
            return $this->jsonError(trans('errors.permissionJson'), 403);
        }

        $note->delete();

        return response()->json([], 204);
    }

    /**
     * Parse @Username mentions from plain text content and fire in-app notifications.
     * Matches `@Name` patterns against user names in the system.
     * On updates, skips users who were already notified for this note.
     */
    protected function processMentions(PageScratchNote $note, string $content, bool $isUpdate): void
    {
        // Match @Word or @First Last style mentions (greedy word boundary match)
        preg_match_all('/@([\w][\w\s]{0,48}[\w]|[\w]+)/u', $content, $matches);
        $rawNames = array_unique($matches[1] ?? []);

        if (empty($rawNames)) {
            return;
        }

        // Find users whose names match
        $mentionedUsers = User::query()
            ->whereIn('name', $rawNames)
            ->get();

        if ($mentionedUsers->isEmpty()) {
            return;
        }

        // On updates, filter out already-notified users
        if ($isUpdate) {
            $previousUserIds = ScratchNoteMentionHistory::query()
                ->where('scratch_note_id', $note->id)
                ->pluck('user_id')
                ->toArray();

            $mentionedUsers = $mentionedUsers->reject(
                fn(User $u) => in_array($u->id, $previousUserIds)
            );
        }

        if ($mentionedUsers->isEmpty()) {
            return;
        }

        // Load the page for permission checks and notification link
        $note->loadMissing('page');
        $page = $note->page;

        if ($page === null) {
            return;
        }

        $initiator = user();
        $service = new InAppNotificationService();

        $now = now();
        $historyRows = [];

        foreach ($mentionedUsers as $recipient) {
            // Don't notify self
            if ($recipient->id === $initiator->id) {
                continue;
            }

            if (!$recipient->can(Permission::ReceiveNotifications)) {
                continue;
            }

            $permissions = new PermissionApplicator($recipient);
            if (!$permissions->checkOwnableUserAccess($page, 'view')) {
                continue;
            }

            try {
                $service->notify($recipient, 'scratch_note_mention', [
                    'title'             => $page->name,
                    'message'           => $initiator->name . ' mentioned you in a scratch note.',
                    'link'              => $page->getUrl(),
                    'entity_id'         => $page->id,
                    'entity_type'       => $page->getMorphClass(),
                    'triggered_by_id'   => $initiator->id,
                    'triggered_by_name' => $initiator->name,
                ]);

                $historyRows[] = [
                    'scratch_note_id' => $note->id,
                    'user_id'         => $recipient->id,
                    'created_at'      => $now,
                ];
            } catch (\Exception $e) {
                Log::error("Failed to create scratch note mention notification for user [id:{$recipient->id}]: {$e->getMessage()}");
            }
        }

        if (!empty($historyRows)) {
            ScratchNoteMentionHistory::query()->insert($historyRows);
        }
    }

    /**
     * Convert a note model to the array shape returned by the API.
     */
    protected function noteToArray(PageScratchNote $note): array
    {
        return [
            'id'         => $note->id,
            'page_id'    => $note->page_id,
            'user_id'    => $note->user_id,
            'user_name'  => $note->user?->name ?? '',
            'content'    => $note->content,
            'created_at' => $note->created_at?->toIso8601String(),
            'updated_at' => $note->updated_at?->toIso8601String(),
        ];
    }
}
