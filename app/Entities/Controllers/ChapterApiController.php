<?php

namespace BookStack\Entities\Controllers;

use BookStack\Entities\Models\Book;
use BookStack\Entities\Models\Chapter;
use BookStack\Entities\Queries\ChapterQueries;
use BookStack\Entities\Queries\EntityQueries;
use BookStack\Entities\Repos\ChapterRepo;
use BookStack\Entities\Tools\Cloner;
use BookStack\Exceptions\MoveOperationException;
use BookStack\Exceptions\PermissionsException;
use BookStack\Http\ApiController;
use BookStack\Permissions\Permission;
use Exception;
use Illuminate\Http\Request;

class ChapterApiController extends ApiController
{
    protected array $rules = [
        'create' => [
            'book_id'             => ['required', 'integer'],
            'name'                => ['required', 'string', 'max:255'],
            'description'         => ['string', 'max:1900'],
            'description_html'    => ['string', 'max:2000'],
            'tags'                => ['array'],
            'priority'            => ['integer'],
            'default_template_id' => ['nullable', 'integer'],
        ],
        'update' => [
            'book_id'             => ['integer'],
            'name'                => ['string', 'min:1', 'max:255'],
            'description'         => ['string', 'max:1900'],
            'description_html'    => ['string', 'max:2000'],
            'tags'                => ['array'],
            'priority'            => ['integer'],
            'default_template_id' => ['nullable', 'integer'],
        ],
    ];

    public function __construct(
        protected ChapterRepo $chapterRepo,
        protected ChapterQueries $queries,
        protected EntityQueries $entityQueries,
        protected Cloner $cloner,
    ) {
    }

    /**
     * Get a listing of chapters visible to the user.
     */
    public function list()
    {
        $chapters = $this->queries->visibleForList()
            ->addSelect(['created_by', 'updated_by']);

        return $this->apiListingResponse($chapters, [
            'id', 'book_id', 'name', 'slug', 'description', 'priority',
            'created_at', 'updated_at', 'created_by', 'updated_by', 'owned_by',
        ]);
    }

    /**
     * Create a new chapter in the system.
     */
    public function create(Request $request)
    {
        $requestData = $this->validate($request, $this->rules['create']);

        $bookId = $request->get('book_id');
        $book = $this->entityQueries->books->findVisibleByIdOrFail(intval($bookId));
        $this->checkOwnablePermission(Permission::ChapterCreate, $book);

        $chapter = $this->chapterRepo->create($requestData, $book);

        return response()->json($this->forJsonDisplay($chapter));
    }

    /**
     * View the details of a single chapter.
     */
    public function read(string $id)
    {
        $chapter = $this->queries->findVisibleByIdOrFail(intval($id));
        $chapter = $this->forJsonDisplay($chapter);

        $chapter->load(['createdBy', 'updatedBy', 'ownedBy']);

        // Note: More fields than usual here, for backwards compatibility,
        // due to previously accidentally including more fields that desired.
        $pages = $this->entityQueries->pages->visibleForChapterList($chapter->id)
            ->addSelect(['created_by', 'updated_by', 'revision_count', 'editor'])
            ->get();
        $chapter->setRelation('pages', $pages);

        return response()->json($chapter);
    }

    /**
     * Update the details of a single chapter.
     * Providing a 'book_id' property will essentially move the chapter
     * into that parent element if you have permissions to do so.
     */
    public function update(Request $request, string $id)
    {
        $requestData = $this->validate($request, $this->rules()['update']);
        $chapter = $this->queries->findVisibleByIdOrFail(intval($id));
        $this->checkOwnablePermission(Permission::ChapterUpdate, $chapter);

        if ($request->has('book_id') && $chapter->book_id !== (intval($requestData['book_id']) ?: null)) {
            $this->checkOwnablePermission(Permission::ChapterDelete, $chapter);

            try {
                $this->chapterRepo->move($chapter, "book:{$requestData['book_id']}");
            } catch (Exception $exception) {
                if ($exception instanceof PermissionsException) {
                    $this->showPermissionError();
                }

                return $this->jsonError(trans('errors.selected_book_not_found'));
            }
        }

        $updatedChapter = $this->chapterRepo->update($chapter, $requestData);

        return response()->json($this->forJsonDisplay($updatedChapter));
    }

    /**
     * Move a chapter into a new parent book.
     * The target must be provided as a string in the format "book:<id>".
     * Requires chapter-update and chapter-delete permissions on the chapter, and
     * chapter-create permission on the target book.
     *
     * @throws MoveOperationException
     * @throws PermissionsException
     */
    public function move(Request $request, string $id)
    {
        $this->validate($request, [
            'target' => ['required', 'string'],
        ]);

        $chapter = $this->queries->findVisibleByIdOrFail(intval($id));
        $this->checkOwnablePermission(Permission::ChapterUpdate, $chapter);
        $this->checkOwnablePermission(Permission::ChapterDelete, $chapter);

        try {
            $this->chapterRepo->move($chapter, $request->get('target'));
        } catch (PermissionsException $exception) {
            $this->showPermissionError();
        } catch (MoveOperationException $exception) {
            return $this->jsonError(trans('errors.selected_book_not_found'), 422);
        }

        $chapter->refresh();

        return response()->json($this->forJsonDisplay($chapter));
    }

    /**
     * Copy a chapter into the given parent book.
     * The target must be provided as a string in the format "book:<id>".
     * If no name is provided the original chapter name will be used.
     * Returns the new chapter with a 201 status code.
     */
    public function copy(Request $request, string $id)
    {
        $this->validate($request, [
            'target' => ['required', 'string'],
            'name'   => ['nullable', 'string', 'max:255'],
        ]);

        $chapter = $this->queries->findVisibleByIdOrFail(intval($id));

        $targetIdentifier = $request->get('target');
        $newParentBook = $this->entityQueries->findVisibleByStringIdentifier($targetIdentifier);

        if (!$newParentBook instanceof Book) {
            return $this->jsonError(trans('errors.selected_book_not_found'), 422);
        }

        $this->checkOwnablePermission(Permission::ChapterCreate, $newParentBook);

        $newName = $request->get('name') ?: $chapter->name;
        $chapterCopy = $this->cloner->cloneChapter($chapter, $newParentBook, $newName);

        return response()->json($this->forJsonDisplay($chapterCopy), 201);
    }

    /**
     * Delete a chapter.
     * This will typically send the chapter to the recycle bin.
     */
    public function delete(string $id)
    {
        $chapter = $this->queries->findVisibleByIdOrFail(intval($id));
        $this->checkOwnablePermission(Permission::ChapterDelete, $chapter);

        $this->chapterRepo->destroy($chapter);

        return response('', 204);
    }

    protected function forJsonDisplay(Chapter $chapter): Chapter
    {
        $chapter = clone $chapter;
        $chapter->unsetRelations()->refresh();

        $chapter->load(['tags']);
        $chapter->makeVisible('description_html');
        $chapter->setAttribute('description_html', $chapter->descriptionInfo()->getHtml());

        /** @var Book $book */
        $book = $chapter->book()->first();
        $chapter->setAttribute('book_slug', $book->slug);

        return $chapter;
    }
}
