<?php

namespace BookStack\Activity\Models;

use BookStack\App\Model;
use BookStack\Entities\Models\Page;
use BookStack\Users\Models\User;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

/**
 * @property int    $id
 * @property int    $page_id
 * @property int    $user_id
 * @property string $content
 */
class PageScratchNote extends Model
{
    protected $fillable = ['page_id', 'user_id', 'content'];

    public $timestamps = true;

    /**
     * Get the user that authored this note.
     *
     * @return BelongsTo<User, $this>
     */
    public function user(): BelongsTo
    {
        return $this->belongsTo(User::class);
    }

    /**
     * Get the page this note belongs to.
     *
     * @return BelongsTo<Page, $this>
     */
    public function page(): BelongsTo
    {
        return $this->belongsTo(Page::class);
    }
}
