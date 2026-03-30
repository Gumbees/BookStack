<?php

declare(strict_types=1);

namespace BookStack\References;

use BookStack\Entities\Models\Entity;
use BookStack\Entities\Models\Page;
use BookStack\Entities\Queries\EntityQueries;
use BookStack\Http\ApiController;
use Illuminate\Http\JsonResponse;
use Illuminate\Support\Str;

class BacklinkApiController extends ApiController
{
    public function __construct(
        protected EntityQueries $queries,
        protected ReferenceFetcher $referenceFetcher,
    ) {
    }

    /**
     * Get the backlinks (linked and unlinked mentions) for the given entity.
     * 'type' must be one of: page, chapter, book, bookshelf (or shelf).
     * 'linked' items are pages/entities with tracked reference links pointing to this entity.
     * 'unlinked' items are pages whose text mentions the entity name without a reference link.
     */
    public function show(string $type, string $id): JsonResponse
    {
        $resolvedType = $type === 'shelf' ? 'bookshelf' : $type;

        $entity = $this->queries->findVisibleById($resolvedType, (int) $id);
        if ($entity === null) {
            return $this->jsonError('Entity not found', 404);
        }

        $linked = $this->getLinkedMentions($entity);
        $unlinked = $this->getUnlinkedMentions($entity, $linked['excludeIds']);

        return response()->json([
            'linked'         => $linked['items'],
            'unlinked'       => $unlinked,
            'linked_count'   => count($linked['items']),
            'unlinked_count' => count($unlinked),
        ]);
    }

    /**
     * Get entities that contain tracked reference links to the given entity.
     *
     * @return array{items: list<array<string, mixed>>, excludeIds: int[]}
     */
    protected function getLinkedMentions(Entity $entity): array
    {
        $references = $this->referenceFetcher->getReferencesToEntity($entity, false);

        $excludePageIds = [];
        $items = [];

        foreach ($references->take(10) as $reference) {
            /** @var Entity|null $from */
            $from = $reference->from;
            if ($from === null) {
                continue;
            }

            if ($from instanceof Page) {
                $excludePageIds[] = $from->id;
            }

            $items[] = $this->formatEntity($from, $entity->name);
        }

        return ['items' => array_values($items), 'excludeIds' => $excludePageIds];
    }

    /**
     * Get pages whose text mentions the entity name without a tracked reference link.
     *
     * @param int[] $excludePageIds
     * @return list<array<string, mixed>>
     */
    protected function getUnlinkedMentions(Entity $entity, array $excludePageIds): array
    {
        $name = $entity->name;
        if (strlen(trim($name)) < 2) {
            return [];
        }

        if ($entity instanceof Page) {
            $excludePageIds[] = $entity->id;
        }

        $pages = $this->queries->pages->visibleForList()
            ->where('entity_page_data.text', 'like', '%' . $name . '%')
            ->whereNotIn('entities.id', $excludePageIds)
            ->limit(10)
            ->get();

        $items = [];
        foreach ($pages as $page) {
            $items[] = $this->formatEntity($page, $name);
        }

        return $items;
    }

    /**
     * Format an entity into the shape returned by the API.
     *
     * @return array<string, mixed>
     */
    protected function formatEntity(Entity $entity, string $highlightTerm): array
    {
        return [
            'id'      => $entity->id,
            'type'    => $entity->getMorphClass(),
            'name'    => $entity->name,
            'url'     => $entity->getUrl(),
            'excerpt' => $this->extractExcerpt($entity, $highlightTerm),
        ];
    }

    /**
     * Get an appropriate plaintext content string for excerpt generation.
     */
    protected function getEntityText(Entity $entity): string
    {
        if ($entity instanceof Page) {
            return $entity->getAttribute('text') ?? '';
        }

        return $entity->getAttribute('description') ?? '';
    }

    /**
     * Extract a short excerpt from the entity's content around the first occurrence
     * of the given term.
     */
    protected function extractExcerpt(Entity $entity, string $term): string
    {
        $text = $this->getEntityText($entity);
        if ($text === '' || $term === '') {
            return '';
        }

        $pos = stripos($text, $term);

        if ($pos === false) {
            return Str::limit($text, 120);
        }

        $termLen = strlen($term);
        $contextPad = 60;
        $start = max(0, $pos - $contextPad);
        $end = min(strlen($text), $pos + $termLen + $contextPad);
        $snippet = substr($text, $start, $end - $start);

        $prefix = $start > 0 ? '...' : '';
        $suffix = $end < strlen($text) ? '...' : '';

        return $prefix . $snippet . $suffix;
    }
}
