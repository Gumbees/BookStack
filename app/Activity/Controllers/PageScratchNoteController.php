<?php

namespace BookStack\Activity\Controllers;

use BookStack\Activity\Models\PageScratchNote;
use BookStack\Entities\Queries\PageQueries;
use BookStack\Http\Controller;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;

class PageScratchNoteController extends Controller
{
    public function __construct(
        protected PageQueries $pageQueries,
    ) {
    }

    /**
     * Get the current user's scratch note for the given page.
     * Creates an empty record if none exists yet.
     */
    public function show(int $pageId): JsonResponse
    {
        $this->preventGuestAccess();

        $page = $this->pageQueries->findVisibleById($pageId);
        if ($page === null) {
            return $this->jsonError('Page not found', 404);
        }

        $note = PageScratchNote::firstOrNew(
            ['user_id' => user()->id, 'page_id' => $pageId],
            ['content' => null],
        );

        return response()->json([
            'content'    => $note->content ?? '',
            'updated_at' => $note->updated_at?->toIso8601String(),
        ]);
    }

    /**
     * Save the current user's scratch note for the given page.
     */
    public function update(Request $request, int $pageId): JsonResponse
    {
        $this->preventGuestAccess();

        $page = $this->pageQueries->findVisibleById($pageId);
        if ($page === null) {
            return $this->jsonError('Page not found', 404);
        }

        $input = $this->validate($request, [
            'content' => ['string', 'nullable'],
        ]);

        $note = PageScratchNote::firstOrNew([
            'user_id' => user()->id,
            'page_id' => $pageId,
        ]);

        $note->fill(['content' => $input['content'] ?? null]);
        $note->updated_at = now();
        $note->save();

        return response()->json([
            'updated_at' => $note->updated_at->toIso8601String(),
        ]);
    }
}
