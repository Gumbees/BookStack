<?php

declare(strict_types=1);

namespace BookStack\Activity\Controllers;

use BookStack\Activity\Models\Watch;
use BookStack\Activity\Tools\UserEntityWatchOptions;
use BookStack\Activity\WatchLevels;
use BookStack\Entities\EntityProvider;
use BookStack\Http\ApiController;
use BookStack\Permissions\Permission;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Http\Response;

class FollowApiController extends ApiController
{
    /**
     * Map API-facing type names to EntityProvider keys.
     * The API uses 'shelf' while the EntityProvider uses 'bookshelf'.
     */
    protected array $typeMap = [
        'shelf'     => 'bookshelf',
        'bookshelf' => 'bookshelf',
        'book'      => 'book',
        'chapter'   => 'chapter',
        'page'      => 'page',
    ];

    public function __construct(
        protected EntityProvider $entities,
    ) {
    }

    /**
     * Get a paginated listing of the current user's follows.
     * Returns all entities the current API user is actively following (any non-default level).
     */
    public function list(Request $request): JsonResponse
    {
        $this->checkPermission(Permission::ReceiveNotifications);

        $query = Watch::query()
            ->where('user_id', '=', user()->id)
            ->where('level', '>', WatchLevels::DEFAULT)
            ->orderBy('created_at', 'desc');

        $total = $query->count();

        $offset = max(0, (int) $request->get('offset', 0));
        $count = max(1, min(500, (int) $request->get('count', 100)));

        $watches = $query->with('watchable')->skip($offset)->take($count)->get();

        $data = $watches->map(function (Watch $watch) {
            $entityType = $watch->watchable_type;
            $entity = $watch->watchable;

            return [
                'id'           => $watch->id,
                'entity_type'  => $entityType,
                'entity_id'    => $watch->watchable_id,
                'entity_name'  => $entity?->name ?? '',
                'entity_url'   => $entity ? $entity->getUrl() : '',
                'level'        => $watch->level,
                'level_label'  => WatchLevels::levelValueToName($watch->level),
                'created_at'   => $watch->created_at?->toIso8601String(),
            ];
        })->values()->all();

        return response()->json([
            'data'  => $data,
            'total' => $total,
        ]);
    }

    /**
     * Get the follow status for a specific entity.
     * Returns whether the current user is following the entity and at what level.
     */
    public function read(string $type, string $id): JsonResponse
    {
        $this->checkPermission(Permission::ReceiveNotifications);

        $entity = $this->findEntityOrFail($type, (int) $id);
        if ($entity === null) {
            return $this->jsonError('Entity not found', 404);
        }

        $watchOptions = new UserEntityWatchOptions(user(), $entity);
        $levelValue = $watchOptions->getWatchLevelValue();
        $isFollowing = $levelValue !== WatchLevels::DEFAULT;

        return response()->json([
            'following'   => $isFollowing,
            'level'       => $isFollowing ? $levelValue : null,
            'level_label' => $isFollowing ? WatchLevels::levelValueToName($levelValue) : null,
        ]);
    }

    /**
     * Follow an entity or update the follow level for the current user.
     * Provide a 'level' integer in the request body:
     * 1 = new content, 2 = updates, 3 = all (comments + updates + new).
     * Use 0 to ignore all notifications for this entity.
     */
    public function update(Request $request, string $type, string $id): JsonResponse
    {
        $this->checkPermission(Permission::ReceiveNotifications);

        $validated = $this->validate($request, [
            'level' => ['required', 'integer', 'min:0', 'max:3'],
        ]);

        $entity = $this->findEntityOrFail($type, (int) $id);
        if ($entity === null) {
            return $this->jsonError('Entity not found', 404);
        }

        $watchOptions = new UserEntityWatchOptions(user(), $entity);
        $watchOptions->updateLevelByValue((int) $validated['level']);

        $levelValue = (int) $validated['level'];

        return response()->json([
            'following'   => true,
            'level'       => $levelValue,
            'level_label' => WatchLevels::levelValueToName($levelValue),
        ]);
    }

    /**
     * Unfollow an entity, removing the follow record for the current user.
     */
    public function destroy(string $type, string $id): Response|JsonResponse
    {
        $this->checkPermission(Permission::ReceiveNotifications);

        $entity = $this->findEntityOrFail($type, (int) $id);
        if ($entity === null) {
            return $this->jsonError('Entity not found', 404);
        }

        $watchOptions = new UserEntityWatchOptions(user(), $entity);
        $watchOptions->updateLevelByValue(WatchLevels::DEFAULT);

        return response('', 204);
    }

    /**
     * Resolve the API type string to an entity instance, returning null for
     * an unrecognised type or a non-visible entity.
     */
    protected function findEntityOrFail(string $type, int $id): ?\BookStack\Entities\Models\Entity
    {
        $providerKey = $this->typeMap[$type] ?? null;
        if ($providerKey === null) {
            return null;
        }

        try {
            $entityModel = $this->entities->get($providerKey);
        } catch (\InvalidArgumentException) {
            return null;
        }

        return $entityModel->newQuery()->scopes(['visible'])->find($id);
    }
}
