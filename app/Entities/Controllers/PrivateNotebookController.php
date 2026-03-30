<?php

namespace BookStack\Entities\Controllers;

use BookStack\Entities\Services\PrivateNotebookService;
use BookStack\Http\Controller;

class PrivateNotebookController extends Controller
{
    public function __construct(
        protected PrivateNotebookService $service,
    ) {
        $this->middleware(function ($request, $next) {
            $this->preventGuestAccess();
            return $next($request);
        });
    }

    /**
     * Redirect to the current user's private notebook, creating it if needed.
     */
    public function index()
    {
        $book = $this->service->getOrCreateForUser(user());

        return redirect($book->getUrl());
    }
}
