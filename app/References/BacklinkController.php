<?php

namespace BookStack\References;

use BookStack\Entities\Models\BookChild;
use BookStack\Entities\Models\Entity;
use BookStack\Entities\Models\Page;
use BookStack\Entities\Queries\EntityQueries;
use BookStack\Http\Controller;
use Illuminate\Http\JsonResponse;
use Illuminate\Support\Str;

class BacklinkController extends Controller
{
    public function __construct(
        protected EntityQueries $queries,
        protected ReferenceFetcher $referenceFetcher,
    ) {
    }

    /**
     * Return linked and unlinked mentions for the given entity, as JSON for the sidebar panel.
     */
    public function show(string $type, int $id): JsonResponse
    {
        $this->preventGuestAccess();

        $entity = $this->queries->findVisibleById($type, $id);
        if ($entity === null) {
            return $this->jsonError('Entity not found', 404);
        }

        $linked = $this->getLinkedMentions($entity);
        $unlinked = $this->getUnlinkedMentions($entity, $linked['excludeIds']);

        return response()->json([
            'linked'   => $linked['items'],
            'unlinked' => $unlinked,
        ]);
    }

    /**
     * Get entities that contain tracked reference links to the given entity.
     * Returns formatted items and a set of page IDs to exclude from unlinked search.
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
     * @param int[] $excludePageIds  IDs of pages already shown as linked mentions
     * @return list<array<string, mixed>>
     */
    protected function getUnlinkedMentions(Entity $entity, array $excludePageIds): array
    {
        $name = $entity->name;
        if (strlen(trim($name)) < 2) {
            return [];
        }

        // Always exclude the entity itself if it's a page
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
     * Format an entity into the shape the frontend panel expects.
     *
     * @return array<string, mixed>
     */
    protected function formatEntity(Entity $entity, string $highlightTerm): array
    {
        return [
            'id'         => $entity->id,
            'type'       => $entity->getMorphClass(),
            'name'       => $entity->name,
            'url'        => $entity->getUrl(),
            'breadcrumb' => $this->getBreadcrumb($entity),
            'excerpt'    => $this->extractExcerpt($entity, $highlightTerm),
        ];
    }

    /**
     * Build a short breadcrumb string showing the entity's parent path.
     */
    protected function getBreadcrumb(Entity $entity): string
    {
        if (!($entity instanceof BookChild)) {
            return '';
        }

        $parts = [];

        $book = $entity->book;
        if ($book) {
            $parts[] = $book->name;
        }

        if ($entity instanceof Page && $entity->chapter_id) {
            $chapter = $entity->chapter;
            if ($chapter) {
                $parts[] = $chapter->name;
            }
        }

        return implode(' > ', $parts);
    }

    /**
     * Get an appropriate plaintext content string for excerpt generation.
     */
    protected function getEntityText(Entity $entity): string
    {
        // Pages have a 'text' attribute from the entity_page_data join
        if ($entity instanceof Page) {
            return $entity->getAttribute('text') ?? '';
        }

        // Books and chapters have 'description' from entity_container_data join
        return $entity->getAttribute('description') ?? '';
    }

    /**
     * Extract a short excerpt from the entity's content around the first occurrence
     * of the given term. The term is wrapped in a highlight mark.
     */
    protected function extractExcerpt(Entity $entity, string $term): string
    {
        $text = $this->getEntityText($entity);
        if ($text === '' || $term === '') {
            return '';
        }

        $pos = stripos($text, $term);

        if ($pos === false) {
            return e(Str::limit($text, 120));
        }

        $termLen = strlen($term);
        $contextPad = 60;
        $start = max(0, $pos - $contextPad);
        $end = min(strlen($text), $pos + $termLen + $contextPad);
        $snippet = substr($text, $start, $end - $start);

        $prefix = $start > 0 ? '...' : '';
        $suffix = $end < strlen($text) ? '...' : '';
        $snippet = $prefix . $snippet . $suffix;

        // Escape HTML then highlight the search term
        $escaped = e($snippet);
        $highlighted = preg_replace(
            '/' . preg_quote(e($term), '/') . '/i',
            '<mark class="backlink-highlight">$0</mark>',
            $escaped
        );

        return $highlighted ?? $escaped;
    }
}
