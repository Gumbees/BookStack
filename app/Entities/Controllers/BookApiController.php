<?php

namespace BookStack\Entities\Controllers;

use BookStack\Activity\ActivityType;
use BookStack\Api\ApiEntityListFormatter;
use BookStack\Entities\Models\Book;
use BookStack\Entities\Models\Chapter;
use BookStack\Entities\Models\Entity;
use BookStack\Entities\Queries\BookQueries;
use BookStack\Entities\Queries\BookshelfQueries;
use BookStack\Entities\Queries\EntityQueries;
use BookStack\Entities\Queries\PageQueries;
use BookStack\Entities\Repos\BookRepo;
use BookStack\Entities\Models\Bookshelf;
use BookStack\Entities\Tools\BookContents;
use BookStack\Entities\Tools\Cloner;
use BookStack\Facades\Activity;
use BookStack\Http\ApiController;
use BookStack\Permissions\Permission;
use BookStack\Sorting\BookSortMap;
use BookStack\Sorting\BookSortMapItem;
use BookStack\Sorting\BookSorter;
use Illuminate\Database\Eloquent\Builder;
use Illuminate\Database\Eloquent\Relations\BelongsToMany;
use Illuminate\Http\Request;
use Illuminate\Validation\ValidationException;

class BookApiController extends ApiController
{
    public function __construct(
        protected BookRepo $bookRepo,
        protected BookQueries $queries,
        protected PageQueries $pageQueries,
        protected BookshelfQueries $shelfQueries,
        protected EntityQueries $entityQueries,
        protected Cloner $cloner,
    ) {
    }

    /**
     * Get a listing of books visible to the user.
     */
    public function list()
    {
        $books = $this->queries
            ->visibleForList()
            ->with(['cover:id,name,url'])
            ->addSelect(['created_by', 'updated_by']);

        return $this->apiListingResponse($books, [
            'id', 'name', 'slug', 'description', 'created_at', 'updated_at', 'created_by', 'updated_by', 'owned_by',
        ]);
    }

    /**
     * Create a new book in the system.
     * An optional shelf_id can be provided to place the book on a shelf upon creation.
     * The cover image of a book can be set by sending a file via an 'image' property within a 'multipart/form-data' request.
     * If the 'image' property is null then the book cover image will be removed.
     *
     * @throws ValidationException
     */
    public function create(Request $request)
    {
        $this->checkPermission(Permission::BookCreateAll);
        $requestData = $this->validate($request, $this->rules()['create']);

        $book = $this->bookRepo->create($requestData);

        $shelfId = $requestData['shelf_id'] ?? null;
        if ($shelfId) {
            $shelf = $this->shelfQueries->findVisibleByIdOrFail(intval($shelfId));
            $this->checkOwnablePermission(Permission::BookshelfUpdate, $shelf);
            $shelf->appendBook($book);
            Activity::add(ActivityType::BOOKSHELF_UPDATE, $shelf);
        }

        return response()->json($this->forJsonDisplay($book));
    }

    /**
     * View the details of a single book.
     * The response data will contain a 'content' property listing the chapter and pages directly within, in
     * the same structure as you'd see within the BookStack interface when viewing a book. Top-level
     * contents will have a 'type' property to distinguish between pages and chapters.
     */
    public function read(string $id)
    {
        $book = $this->queries->findVisibleByIdOrFail(intval($id));
        $book = $this->forJsonDisplay($book);
        $book->load([
            'createdBy',
            'updatedBy',
            'ownedBy',
            'shelves' => function (BelongsToMany $query) {
                $query->select(['id', 'name', 'slug'])->scopes('visible');
            }
        ]);

        $contents = (new BookContents($book))->getTree(true, false)->all();
        $contentsApiData = (new ApiEntityListFormatter($contents))
            ->withType()
            ->withField('pages', function (Entity $entity) {
                if ($entity instanceof Chapter) {
                    $pages = $this->pageQueries->visibleForChapterList($entity->id)->get()->all();
                    return (new ApiEntityListFormatter($pages))->format();
                }
                return null;
            })->format();
        $book->setAttribute('contents', $contentsApiData);

        return response()->json($book);
    }

    /**
     * Update the details of a single book.
     * The cover image of a book can be set by sending a file via an 'image' property within a 'multipart/form-data' request.
     * If the 'image' property is null then the book cover image will be removed.
     *
     * @throws ValidationException
     */
    public function update(Request $request, string $id)
    {
        $book = $this->queries->findVisibleByIdOrFail(intval($id));
        $this->checkOwnablePermission(Permission::BookUpdate, $book);

        $requestData = $this->validate($request, $this->rules()['update']);
        $book = $this->bookRepo->update($book, $requestData);

        return response()->json($this->forJsonDisplay($book));
    }

    /**
     * Delete a single book.
     * This will typically send the book to the recycle bin.
     *
     * @throws \Exception
     */
    public function delete(string $id)
    {
        $book = $this->queries->findVisibleByIdOrFail(intval($id));
        $this->checkOwnablePermission(Permission::BookDelete, $book);

        $this->bookRepo->destroy($book);

        return response('', 204);
    }

    /**
     * Move a book to a target shelf, or detach it from all shelves.
     * The target must be provided as a string in the format "shelf:<id>".
     * To detach the book from all shelves without placing it on another, pass null or "none".
     * Requires book-update on the book and bookshelf-update on the target shelf.
     *
     * @throws ValidationException
     */
    public function move(Request $request, string $id)
    {
        $this->validate($request, [
            'target' => ['nullable', 'string'],
        ]);

        $book = $this->queries->findVisibleByIdOrFail(intval($id));
        $this->checkOwnablePermission(Permission::BookUpdate, $book);

        $targetRaw = $request->get('target');

        // Detach from all shelves when target is null or "none"
        if (!$targetRaw || strtolower($targetRaw) === 'none') {
            $currentShelves = $book->shelves()->scopes('visible')->get();
            foreach ($currentShelves as $shelf) {
                $this->checkOwnablePermission(Permission::BookshelfUpdate, $shelf);
                $shelf->books()->detach($book->id);
                Activity::add(ActivityType::BOOKSHELF_UPDATE, $shelf);
            }

            return response()->json($this->forJsonDisplay($book));
        }

        $targetEntity = $this->entityQueries->findVisibleByStringIdentifier($targetRaw);

        if (!$targetEntity instanceof Bookshelf) {
            return $this->jsonError(trans('errors.bookshelf_not_found'), 422);
        }

        $this->checkOwnablePermission(Permission::BookshelfUpdate, $targetEntity);

        // Detach from existing shelves then attach to target
        $currentShelves = $book->shelves()->scopes('visible')->get();
        foreach ($currentShelves as $currentShelf) {
            if ($currentShelf->id !== $targetEntity->id) {
                $this->checkOwnablePermission(Permission::BookshelfUpdate, $currentShelf);
                $currentShelf->books()->detach($book->id);
                Activity::add(ActivityType::BOOKSHELF_UPDATE, $currentShelf);
            }
        }

        $targetEntity->appendBook($book);
        Activity::add(ActivityType::BOOKSHELF_UPDATE, $targetEntity);

        return response()->json($this->forJsonDisplay($book));
    }

    /**
     * Copy a book, duplicating all its chapters and pages.
     * If no name is provided the original book name will be used.
     * Requires book-create-all permission.
     * Returns the new book with a 201 status code.
     */
    public function copy(Request $request, string $id)
    {
        $this->validate($request, [
            'name' => ['nullable', 'string', 'max:255'],
        ]);

        $book = $this->queries->findVisibleByIdOrFail(intval($id));
        $this->checkOwnablePermission(Permission::BookView, $book);
        $this->checkPermission(Permission::BookCreateAll);

        $newName = $request->get('name') ?: $book->name;
        $bookCopy = $this->cloner->cloneBook($book, $newName);

        return response()->json($this->forJsonDisplay($bookCopy), 201);
    }

    /**
     * Update the sort order of content within a book.
     * Provide an array of items in the "order" property, each with an "id", "type" ("page" or "chapter"),
     * "sort" (integer priority), and optionally "chapter_id" (integer, for pages within a chapter).
     * Items not listed retain their current position and book assignment.
     * Requires book-update permission. Items that the user lacks permission to move are silently skipped.
     *
     * @throws ValidationException
     */
    public function sort(Request $request, BookSorter $sorter, string $id)
    {
        $this->validate($request, [
            'order'              => ['required', 'array'],
            'order.*.id'         => ['required', 'integer'],
            'order.*.type'       => ['required', 'string', 'in:page,chapter'],
            'order.*.sort'       => ['required', 'integer'],
            'order.*.chapter_id' => ['nullable', 'integer'],
        ]);

        $book = $this->queries->findVisibleByIdOrFail(intval($id));
        $this->checkOwnablePermission(Permission::BookUpdate, $book);

        $sortMap = new BookSortMap();
        foreach ($request->get('order') as $item) {
            $sortMap->addItem(new BookSortMapItem(
                intval($item['id']),
                intval($item['sort']),
                isset($item['chapter_id']) ? (intval($item['chapter_id']) ?: null) : null,
                $item['type'],
                $book->id,
            ));
        }

        $booksInvolved = $sorter->sortUsingMap($sortMap);
        foreach ($booksInvolved as $bookInvolved) {
            Activity::add(ActivityType::BOOK_SORT, $bookInvolved);
        }

        return response()->json($this->forJsonDisplay($book->refresh()));
    }

    protected function forJsonDisplay(Book $book): Book
    {
        $book = clone $book;
        $book->unsetRelations()->refresh();

        $book->load(['tags']);
        $book->makeVisible(['cover', 'description_html'])
            ->setAttribute('description_html', $book->descriptionInfo()->getHtml())
            ->setAttribute('cover', $book->coverInfo()->getImage());

        return $book;
    }

    protected function rules(): array
    {
        return [
            'create' => [
                'name'                => ['required', 'string', 'max:255'],
                'description'         => ['string', 'max:1900'],
                'description_html'    => ['string', 'max:2000'],
                'tags'                => ['array'],
                'image'               => array_merge(['nullable'], $this->getImageValidationRules()),
                'default_template_id' => ['nullable', 'integer'],
                'shelf_id'            => ['nullable', 'integer'],
            ],
            'update' => [
                'name'                => ['string', 'min:1', 'max:255'],
                'description'         => ['string', 'max:1900'],
                'description_html'    => ['string', 'max:2000'],
                'tags'                => ['array'],
                'image'               => array_merge(['nullable'], $this->getImageValidationRules()),
                'default_template_id' => ['nullable', 'integer'],
            ],
        ];
    }
}
