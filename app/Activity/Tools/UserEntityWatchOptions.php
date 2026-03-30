<?php

namespace BookStack\Activity\Tools;

use BookStack\Activity\Models\Watch;
use BookStack\Activity\WatchLevels;
use BookStack\Entities\Models\Book;
use BookStack\Entities\Models\BookChild;
use BookStack\Entities\Models\Entity;
use BookStack\Entities\Models\Page;
use BookStack\Permissions\Permission;
use BookStack\Users\Models\User;
use Illuminate\Database\Eloquent\Builder;

class UserEntityWatchOptions
{
    protected ?array $watchMap = null;

    public function __construct(
        protected User $user,
        protected Entity $entity,
    ) {
    }

    public function canWatch(): bool
    {
        return $this->user->can(Permission::ReceiveNotifications) && !$this->user->isGuest();
    }

    public function getWatchLevel(): string
    {
        return WatchLevels::levelValueToName($this->getWatchLevelValue());
    }

    public function isWatching(): bool
    {
        return $this->getWatchLevelValue() !== WatchLevels::DEFAULT;
    }

    public function getWatchedParent(): ?WatchedParentDetails
    {
        $watchMap = $this->getWatchMap();
        unset($watchMap[$this->entity->getMorphClass()]);

        if (isset($watchMap['chapter'])) {
            return new WatchedParentDetails('chapter', $watchMap['chapter']);
        }

        if (isset($watchMap['book'])) {
            return new WatchedParentDetails('book', $watchMap['book']);
        }

        if (isset($watchMap['bookshelf'])) {
            return new WatchedParentDetails('bookshelf', $watchMap['bookshelf']);
        }

        return null;
    }

    public function updateLevelByName(string $level): void
    {
        $levelValue = WatchLevels::levelNameToValue($level);
        $this->updateLevelByValue($levelValue);
    }

    public function updateLevelByValue(int $level): void
    {
        if ($level < 0) {
            $this->remove();
            return;
        }

        $this->updateLevel($level);
    }

    public function getWatchMap(): array
    {
        if (!is_null($this->watchMap)) {
            return $this->watchMap;
        }

        $entities = [$this->entity];
        if ($this->entity instanceof BookChild) {
            $entities[] = $this->entity->book;
        }
        if ($this->entity instanceof Page && $this->entity->chapter) {
            $entities[] = $this->entity->chapter;
        }

        // Include shelves that contain the relevant book so shelf-level follows
        // appear as a watched parent in the UI.
        $book = null;
        if ($this->entity instanceof Book) {
            $book = $this->entity;
        } elseif ($this->entity instanceof BookChild) {
            $book = $this->entity->book;
        }

        if ($book) {
            foreach ($book->shelves()->get() as $shelf) {
                $entities[] = $shelf;
            }
        }

        $query = Watch::query()
            ->where('user_id', '=', $this->user->id)
            ->where(function (Builder $subQuery) use ($entities) {
                foreach ($entities as $entity) {
                    $subQuery->orWhere(function (Builder $whereQuery) use ($entity) {
                        $whereQuery->where('watchable_type', '=', $entity->getMorphClass())
                        ->where('watchable_id', '=', $entity->id);
                    });
                }
            });

        // For shelves we want to keep only the highest-level (most specific) match per type.
        // The watchMap is keyed by watchable_type, so shelf watches are stored under 'bookshelf'.
        // If a book has multiple shelves with different watch levels, we take the highest level.
        $results = $query->get(['watchable_type', 'watchable_id', 'level']);

        $watchMap = [];
        foreach ($results as $result) {
            $type = $result->watchable_type;
            // For shelves, keep the maximum level across all parent shelves
            if ($type === 'bookshelf') {
                if (!isset($watchMap[$type]) || $result->level > $watchMap[$type]) {
                    $watchMap[$type] = $result->level;
                }
            } else {
                $watchMap[$type] = $result->level;
            }
        }

        $this->watchMap = $watchMap;

        return $this->watchMap;
    }

    public function getWatchLevelValue(): int
    {
        return $this->getWatchMap()[$this->entity->getMorphClass()] ?? WatchLevels::DEFAULT;
    }

    protected function updateLevel(int $levelValue): void
    {
        Watch::query()->updateOrCreate([
            'watchable_id' => $this->entity->id,
            'watchable_type' => $this->entity->getMorphClass(),
            'user_id' => $this->user->id,
        ], [
            'level' => $levelValue,
        ]);
        $this->watchMap = null;
    }

    protected function remove(): void
    {
        $this->entityQuery()->delete();
        $this->watchMap = null;
    }

    protected function entityQuery(): Builder
    {
        return Watch::query()->where('watchable_id', '=', $this->entity->id)
            ->where('watchable_type', '=', $this->entity->getMorphClass())
            ->where('user_id', '=', $this->user->id);
    }
}
