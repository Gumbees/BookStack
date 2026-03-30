<?php

namespace BookStack\Entities\Services;

use BookStack\Entities\Models\Book;
use BookStack\Entities\Models\Chapter;
use BookStack\Entities\Models\Page;
use BookStack\Entities\Tools\BookContents;
use BookStack\Entities\Tools\PageEditorType;
use BookStack\Entities\Tools\SlugGenerator;
use BookStack\Users\Models\User;
use Carbon\Carbon;
use Illuminate\Support\Str;

class PrivateJournalService
{
    public function __construct(
        protected SlugGenerator $slugGenerator,
    ) {
    }

    /**
     * Get or lazily create the private journal book for the given user.
     * The book ID is stored in user settings so it can be distinguished
     * from the user's private notebook (both have is_private = true).
     */
    public function getOrCreateForUser(User $user): Book
    {
        $storedId = intval(setting()->getUser($user, 'journal_book_id', 0));

        if ($storedId > 0) {
            $book = Book::query()
                ->where('id', '=', $storedId)
                ->where('is_private', '=', true)
                ->where('owned_by', '=', $user->id)
                ->first();

            if ($book) {
                return $book;
            }
        }

        $book = new Book();
        $book->name = $user->name . "'s Journal";
        $book->forceFill([
            'created_by'       => $user->id,
            'updated_by'       => $user->id,
            'owned_by'         => $user->id,
            'is_private'       => true,
            'description'      => '',
            'description_html' => '',
        ]);

        $this->slugGenerator->regenerateForEntity($book);
        $book->save();

        setting()->putUser($user, 'journal_book_id', (string) $book->id);

        return $book;
    }

    /**
     * Get or create the chapter for the given YYYY-MM string within the journal book.
     * Chapters are named by month (e.g. "2026-03") and created on demand.
     */
    public function getOrCreateMonthChapter(Book $journal, string $yearMonth): Chapter
    {
        $chapter = Chapter::query()
            ->where('book_id', '=', $journal->id)
            ->where('name', '=', $yearMonth)
            ->first();

        if ($chapter) {
            return $chapter;
        }

        $chapter = new Chapter();
        $chapter->name = $yearMonth;
        $chapter->book_id = $journal->id;
        $chapter->priority = (new BookContents($journal))->getLastPriority() + 1;
        $chapter->forceFill([
            'created_by'       => $journal->owned_by,
            'updated_by'       => $journal->owned_by,
            'owned_by'         => $journal->owned_by,
            'description'      => '',
            'description_html' => '',
        ]);

        $this->slugGenerator->regenerateForEntity($chapter);
        $chapter->save();
        $chapter->rebuildPermissions();
        $chapter->indexForSearch();

        return $chapter;
    }

    /**
     * Get or create today's journal entry page.
     * Creates the month chapter on demand if needed.
     * Returns the page (already saved, non-draft).
     */
    public function getOrCreateTodayPage(Book $journal): Page
    {
        $today = Carbon::now()->format('Y-m-d');
        return $this->getOrCreateDatePage($journal, $today);
    }

    /**
     * Get or create a journal entry page for the given date string (YYYY-MM-DD).
     * Creates the month chapter on demand if needed.
     * Returns the page (already saved, non-draft).
     */
    public function getOrCreateDatePage(Book $journal, string $date): Page
    {
        $yearMonth = Str::substr($date, 0, 7);
        $chapter = $this->getOrCreateMonthChapter($journal, $yearMonth);

        $page = Page::query()
            ->where('book_id', '=', $journal->id)
            ->where('chapter_id', '=', $chapter->id)
            ->where('name', '=', $date)
            ->where('draft', '=', false)
            ->first();

        if ($page) {
            return $page;
        }

        $lastPage = $chapter->pages('desc')->where('draft', '=', false)->first();
        $priority = $lastPage ? $lastPage->priority + 1 : 0;

        $editor = PageEditorType::getSystemDefault();

        $page = new Page();
        $page->name = $date;
        $page->book_id = $journal->id;
        $page->chapter_id = $chapter->id;
        $page->draft = false;
        $page->revision_count = 1;
        $page->priority = $priority;
        $page->editor = $editor->value;
        $page->forceFill([
            'created_by' => $journal->owned_by,
            'updated_by' => $journal->owned_by,
            'owned_by'   => $journal->owned_by,
            'html'       => '',
            'markdown'   => '',
            'text'       => '',
        ]);

        $this->slugGenerator->regenerateForEntity($page);
        $page->save();
        $page->rebuildPermissions();
        $page->indexForSearch();

        return $page;
    }

    /**
     * Check whether the given book is the current user's private journal.
     */
    public function isUsersJournal(Book $book, User $user): bool
    {
        $storedId = intval(setting()->getUser($user, 'journal_book_id', 0));
        return $storedId > 0 && $book->id === $storedId;
    }
}
