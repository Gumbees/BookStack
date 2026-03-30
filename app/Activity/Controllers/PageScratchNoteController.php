<?php

namespace BookStack\Activity\Controllers;

use BookStack\Activity\Models\PageScratchNote;
use BookStack\Entities\Queries\PageQueries;
use BookStack\Http\Controller;
use BookStack\Permissions\Permission;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;

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
