<?php

namespace BookStack\Entities\Controllers;

use BookStack\Activity\ActivityType;
use BookStack\Entities\Models\Bookshelf;
use BookStack\Entities\Queries\BookQueries;
use BookStack\Entities\Queries\BookshelfQueries;
use BookStack\Entities\Repos\BookshelfRepo;
use BookStack\Facades\Activity;
use BookStack\Http\ApiController;
use BookStack\Permissions\Permission;
use Exception;
use Illuminate\Database\Eloquent\Relations\BelongsToMany;
use Illuminate\Http\Request;
use Illuminate\Validation\ValidationException;

class BookshelfApiController extends ApiController
{
    public function __construct(
        protected BookshelfRepo $bookshelfRepo,
        protected BookshelfQueries $queries,
        protected BookQueries $bookQueries,
    ) {
    }

    /**
     * Get a listing of shelves visible to the user.
     */
    public function list()
    {
        $shelves = $this->queries
            ->visibleForList()
            ->with(['cover:id,name,url'])
            ->addSelect(['created_by', 'updated_by']);

        return $this->apiListingResponse($shelves, [
            'id', 'name', 'slug', 'description', 'created_at', 'updated_at', 'created_by', 'updated_by', 'owned_by',
        ]);
    }

    /**
     * Create a new shelf in the system.
     * An array of books IDs can be provided in the request. These
     * will be added to the shelf in the same order as provided.
     * The cover image of a shelf can be set by sending a file via an 'image' property within a 'multipart/form-data' request.
     * If the 'image' property is null then the shelf cover image will be removed.
     *
     * @throws ValidationException
     */
    public function create(Request $request)
    {
        $this->checkPermission(Permission::BookshelfCreateAll);
        $requestData = $this->validate($request, $this->rules()['create']);

        $bookIds = $request->get('books', []);
        $shelf = $this->bookshelfRepo->create($requestData, $bookIds);

        return response()->json($this->forJsonDisplay($shelf));
    }

    /**
     * View the details of a single shelf.
     */
    public function read(string $id)
    {
        $shelf = $this->queries->findVisibleByIdOrFail(intval($id));
        $shelf = $this->forJsonDisplay($shelf);
        $shelf->load([
            'createdBy', 'updatedBy', 'ownedBy',
            'books' => function (BelongsToMany $query) {
                $query->scopes('visible')->get(['id', 'name', 'slug']);
            },
        ]);

        return response()->json($shelf);
    }

    /**
     * Update the details of a single shelf.
     * An array of books IDs can be provided in the request. These
     * will be added to the shelf in the same order as provided and overwrite
     * any existing book assignments.
     * The cover image of a shelf can be set by sending a file via an 'image' property within a 'multipart/form-data' request.
     * If the 'image' property is null then the shelf cover image will be removed.
     *
     * @throws ValidationException
     */
    public function update(Request $request, string $id)
    {
        $shelf = $this->queries->findVisibleByIdOrFail(intval($id));
        $this->checkOwnablePermission(Permission::BookshelfUpdate, $shelf);

        $requestData = $this->validate($request, $this->rules()['update']);
        $bookIds = $request->get('books', null);

        $shelf = $this->bookshelfRepo->update($shelf, $requestData, $bookIds);

        return response()->json($this->forJsonDisplay($shelf));
    }

    /**
     * Delete a single shelf.
     * This will typically send the shelf to the recycle bin.
     *
     * @throws Exception
     */
    public function delete(string $id)
    {
        $shelf = $this->queries->findVisibleByIdOrFail(intval($id));
        $this->checkOwnablePermission(Permission::BookshelfDelete, $shelf);

        $this->bookshelfRepo->destroy($shelf);

        return response('', 204);
    }

    /**
     * Add a book to a shelf.
     * The book will be appended to the end of the shelf's book list.
     * If the book is already on the shelf, the request is a no-op and returns success.
     */
    public function attachBook(string $id, string $bookId)
    {
        $shelf = $this->queries->findVisibleByIdOrFail(intval($id));
        $this->checkOwnablePermission(Permission::BookshelfUpdate, $shelf);

        $book = $this->bookQueries->findVisibleByIdOrFail(intval($bookId));

        $shelf->appendBook($book);
        Activity::add(ActivityType::BOOKSHELF_UPDATE, $shelf);

        return response()->json($this->forJsonDisplay($shelf));
    }

    /**
     * Remove a book from a shelf.
     * This only detaches the book from the shelf; it does not delete the book.
     * If the book is not currently on the shelf, the request is a no-op and returns success.
     * Note: findVisibleByIdOrFail on the book returns 404 for non-visible books, preventing
     * users from detaching books they cannot see — consistent with the updateBooks invariant
     * that preserves non-visible book assignments.
     */
    public function detachBook(string $id, string $bookId)
    {
        $shelf = $this->queries->findVisibleByIdOrFail(intval($id));
        $this->checkOwnablePermission(Permission::BookshelfUpdate, $shelf);

        // findVisibleByIdOrFail ensures users cannot target books they lack view permission
        // on, which aligns with the updateBooks invariant that preserves non-visible books.
        $book = $this->bookQueries->findVisibleByIdOrFail(intval($bookId));

        $shelf->books()->detach($book->id);
        Activity::add(ActivityType::BOOKSHELF_UPDATE, $shelf);

        return response('', 204);
    }

    protected function forJsonDisplay(Bookshelf $shelf): Bookshelf
    {
        $shelf = clone $shelf;
        $shelf->unsetRelations()->refresh();

        $shelf->load(['tags']);
        $shelf->makeVisible(['cover', 'description_html'])
            ->setAttribute('description_html', $shelf->descriptionInfo()->getHtml())
            ->setAttribute('cover', $shelf->coverInfo()->getImage());

        return $shelf;
    }

    protected function rules(): array
    {
        return [
            'create' => [
                'name'             => ['required', 'string', 'max:255'],
                'description'      => ['string', 'max:1900'],
                'description_html' => ['string', 'max:2000'],
                'books'            => ['array'],
                'tags'             => ['array'],
                'image'            => array_merge(['nullable'], $this->getImageValidationRules()),
            ],
            'update' => [
                'name'             => ['string', 'min:1', 'max:255'],
                'description'      => ['string', 'max:1900'],
                'description_html' => ['string', 'max:2000'],
                'books'            => ['array'],
                'tags'             => ['array'],
                'image'            => array_merge(['nullable'], $this->getImageValidationRules()),
            ],
        ];
    }
}
