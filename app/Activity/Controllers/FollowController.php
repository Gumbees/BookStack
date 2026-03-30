<?php

namespace BookStack\Activity\Controllers;

use BookStack\Activity\Tools\UserEntityWatchOptions;
use BookStack\Activity\WatchLevels;
use BookStack\Entities\EntityProvider;
use BookStack\Http\Controller;
use BookStack\Permissions\Permission;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;

class FollowController extends Controller
{
    public function __construct(
        protected EntityProvider $entities,
    ) {
    }

    /**
     * PUT /ajax/follow/{type}/{id}
     * Create or update a follow (watch) record for the given entity.
     */
    public function update(Request $request, string $type, int $id): JsonResponse
    {
        $this->checkPermission(Permission::ReceiveNotifications);
        $this->preventGuestAccess();

        $validated = $this->validate($request, [
            'level' => ['required', 'integer'],
        ]);

        $entity = $this->entities->get($type)->newQuery()->scopes(['visible'])->findOrFail($id);
        $watchOptions = new UserEntityWatchOptions(user(), $entity);
        $watchOptions->updateLevelByValue((int) $validated['level']);

        return response()->json([
            'success' => true,
            'level'   => $validated['level'],
        ]);
    }

    /**
     * DELETE /ajax/follow/{type}/{id}
     * Remove a follow (watch) record for the given entity.
     */
    public function destroy(string $type, int $id): JsonResponse
    {
        $this->checkPermission(Permission::ReceiveNotifications);
        $this->preventGuestAccess();

        $entity = $this->entities->get($type)->newQuery()->scopes(['visible'])->findOrFail($id);
        $watchOptions = new UserEntityWatchOptions(user(), $entity);
        $watchOptions->updateLevelByValue(WatchLevels::DEFAULT);

        return response()->json(['success' => true]);
    }
}
