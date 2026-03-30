<?php

namespace BookStack\Entities\Services;

use BookStack\Entities\Models\Book;
use BookStack\Entities\Tools\SlugGenerator;
use BookStack\Users\Models\User;

class PrivateNotebookService
{
    public function __construct(
        protected SlugGenerator $slugGenerator,
    ) {
    }

    /**
     * Get or lazily create the private notebook for the given user.
     * The book is owned by the user, marked is_private = true, and
     * never receives joint permission rows.
     */
    public function getOrCreateForUser(User $user): Book
    {
        $book = Book::query()
            ->where('is_private', '=', true)
            ->where('owned_by', '=', $user->id)
            ->first();

        if ($book) {
            return $book;
        }

        $book = new Book();
        $book->name = $user->name . "'s Notebook";
        $book->forceFill([
            'created_by'      => $user->id,
            'updated_by'      => $user->id,
            'owned_by'        => $user->id,
            'is_private'      => true,
            'description'     => '',
            'description_html' => '',
        ]);

        $this->slugGenerator->regenerateForEntity($book);
        $book->save();

        return $book;
    }
}
