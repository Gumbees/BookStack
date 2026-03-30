<?php

namespace BookStack\Collaboration;

use BookStack\Entities\Queries\PageQueries;
use BookStack\Http\Controller;
use BookStack\Permissions\Permission;
use Illuminate\Http\JsonResponse;

class CollabController extends Controller
{
    public function __construct(
        protected CollabTokenService $tokenService,
        protected PageQueries $pageQueries,
    ) {
    }

    /**
     * Return a short-lived JWT and WebSocket URL for the collaborative editor.
     * Requires that the current user has edit permission on the page and that
     * collaborative editing is enabled in config.
     */
    public function token(int $pageId): JsonResponse
    {
        if (!config('collab.enabled')) {
            return $this->jsonError('Collaborative editing is not enabled.', 403);
        }

        $page = $this->pageQueries->findVisibleById($pageId);
        if ($page === null) {
            return $this->jsonError('Page not found.', 404);
        }

        if (!userCan(Permission::PageUpdate, $page)) {
            return $this->jsonError('You do not have permission to edit this page.', 403);
        }

        $token = $this->tokenService->generateToken(user(), $pageId);

        return response()->json([
            'ws_url' => config('collab.server_url'),
            'token'  => $token,
        ]);
    }
}
