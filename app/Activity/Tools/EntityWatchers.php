<?php

namespace BookStack\Activity\Tools;

use BookStack\Activity\Models\Watch;
use BookStack\Entities\Models\Book;
use BookStack\Entities\Models\BookChild;
use BookStack\Entities\Models\Bookshelf;
use BookStack\Entities\Models\Entity;
use BookStack\Entities\Models\Page;
use Illuminate\Database\Eloquent\Builder;

class EntityWatchers
{
    /**
     * @var int[]
     */
    protected array $watchers = [];

    /**
     * @var int[]
     */
    protected array $ignorers = [];

    public function __construct(
        protected Entity $entity,
        protected int $watchLevel,
    ) {
        $this->build();
    }

    public function getWatcherUserIds(): array
    {
        return $this->watchers;
    }

    public function isUserIgnoring(int $userId): bool
    {
        return in_array($userId, $this->ignorers);
    }

    protected function build(): void
    {
        $watches = $this->getRelevantWatches();

        // Sort before de-duping so that the order looped below follows shelf -> book -> chapter -> page ordering.
        // Shelf watches have lowest priority (broadest scope), page watches have highest (most specific).
        usort($watches, function (Watch $watchA, Watch $watchB) {
            $order = ['bookshelf', 'book', 'chapter', 'page'];
            $posA = array_search($watchA->watchable_type, $order);
            $posB = array_search($watchB->watchable_type, $order);
            $posA = $posA === false ? 99 : $posA;
            $posB = $posB === false ? 99 : $posB;
            $diff = $posA <=> $posB;
            return $diff === 0 ? ($watchA->user_id <=> $watchB->user_id) : $diff;
        });

        // De-dupe by user id to get their most relevant (most specific) level.
        // Because we sort shelf -> book -> chapter -> page, a later (more specific) entry
        // for the same user will overwrite an earlier (less specific) one.
        $levelByUserId = [];
        foreach ($watches as $watch) {
            $levelByUserId[$watch->user_id] = $watch->level;
        }

        // Populate the class arrays
        $this->watchers = array_keys(array_filter($levelByUserId, fn(int $level) => $level >= $this->watchLevel));
        $this->ignorers = array_keys(array_filter($levelByUserId, fn(int $level) => $level === 0));
    }

    /**
     * @return Watch[]
     */
    protected function getRelevantWatches(): array
    {
        /** @var Entity[] $entitiesInvolved */
        $entitiesInvolved = array_filter([
            $this->entity,
            $this->entity instanceof BookChild ? $this->entity->book : null,
            $this->entity instanceof Page ? $this->entity->chapter : null,
        ]);

        // Also include watches on any shelves that contain the relevant book.
        // This lets shelf-level follows propagate down to books, chapters, and pages.
        $shelfEntities = $this->getShelvesForEntity($this->entity);
        $entitiesInvolved = array_merge(array_values($entitiesInvolved), $shelfEntities);

        $query = Watch::query()->where(function (Builder $query) use ($entitiesInvolved) {
            foreach ($entitiesInvolved as $entity) {
                $query->orWhere(function (Builder $query) use ($entity) {
                    $query->where('watchable_type', '=', $entity->getMorphClass())
                        ->where('watchable_id', '=', $entity->id);
                });
            }
        });

        return $query->get([
            'level', 'watchable_id', 'watchable_type', 'user_id'
        ])->all();
    }

    /**
     * Get the bookshelves that contain the book related to the given entity.
     * Returns an empty array if the entity has no book relationship.
     *
     * @return Bookshelf[]
     */
    protected function getShelvesForEntity(Entity $entity): array
    {
        $book = null;

        if ($entity instanceof Book) {
            $book = $entity;
        } elseif ($entity instanceof BookChild) {
            $book = $entity->book;
        }

        if ($book === null) {
            return [];
        }

        return $book->shelves()->get()->all();
    }
}
