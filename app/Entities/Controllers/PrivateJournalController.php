<?php

namespace BookStack\Entities\Controllers;

use BookStack\Entities\Services\PrivateJournalService;
use BookStack\Http\Controller;

class PrivateJournalController extends Controller
{
    public function __construct(
        protected PrivateJournalService $service,
    ) {
        $this->middleware(function ($request, $next) {
            $this->preventGuestAccess();
            return $next($request);
        });
    }

    /**
     * Redirect to the current user's private journal book, creating it if needed.
     */
    public function index()
    {
        $journal = $this->service->getOrCreateForUser(user());

        return redirect($journal->getUrl());
    }

    /**
     * Get or create today's journal entry page and redirect to it for editing.
     */
    public function today()
    {
        $journal = $this->service->getOrCreateForUser(user());
        $page = $this->service->getOrCreateTodayPage($journal);

        return redirect($page->getUrl('/edit'));
    }

    /**
     * Get or create a journal entry page for the given date and redirect to it for editing.
     */
    public function date(string $date)
    {
        $journal = $this->service->getOrCreateForUser(user());
        $page = $this->service->getOrCreateDatePage($journal, $date);

        return redirect($page->getUrl('/edit'));
    }
}
