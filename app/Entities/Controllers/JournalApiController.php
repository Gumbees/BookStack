<?php

namespace BookStack\Entities\Controllers;

use BookStack\Entities\Models\Page;
use BookStack\Entities\Services\PrivateJournalService;
use BookStack\Http\ApiController;
use Illuminate\Http\JsonResponse;

class JournalApiController extends ApiController
{
    public function __construct(
        protected PrivateJournalService $service,
    ) {
    }

    /**
     * Get the current user's private journal book, creating it if it does not yet exist.
     * Returns the book object.
     */
    public function book(): JsonResponse
    {
        $journal = $this->service->getOrCreateForUser(user());

        return response()->json([
            'id'         => $journal->id,
            'name'       => $journal->name,
            'slug'       => $journal->slug,
            'url'        => $journal->getUrl(),
            'created_at' => $journal->created_at,
            'updated_at' => $journal->updated_at,
        ]);
    }

    /**
     * Get or create today's journal entry page.
     * Returns the page object.
     */
    public function today(): JsonResponse
    {
        $journal = $this->service->getOrCreateForUser(user());
        $page = $this->service->getOrCreateTodayPage($journal);

        return response()->json($this->pageToArray($page));
    }

    /**
     * Get or create a journal entry page for the given date (YYYY-MM-DD format).
     * Returns the page object.
     */
    public function date(string $date): JsonResponse
    {
        if (!preg_match('/^\d{4}-\d{2}-\d{2}$/', $date)) {
            return $this->jsonError(trans('errors.validation'), 422);
        }

        if (!strtotime($date)) {
            return $this->jsonError(trans('errors.validation'), 422);
        }

        $journal = $this->service->getOrCreateForUser(user());
        $page = $this->service->getOrCreateDatePage($journal, $date);

        return response()->json($this->pageToArray($page));
    }

    protected function pageToArray(Page $page): array
    {
        return [
            'id'         => $page->id,
            'name'       => $page->name,
            'slug'       => $page->slug,
            'book_id'    => $page->book_id,
            'chapter_id' => $page->chapter_id,
            'url'        => $page->getUrl(),
            'edit_url'   => $page->getUrl('/edit'),
            'created_at' => $page->created_at,
            'updated_at' => $page->updated_at,
        ];
    }
}
