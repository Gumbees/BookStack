<?php

namespace BookStack\Activity\Models;

use BookStack\App\Model;
use BookStack\Entities\Models\Page;
use BookStack\Users\Models\User;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

/**
 * @property int         $id
 * @property int         $user_id
 * @property int         $page_id
 * @property string|null $content
 * @property string|null $updated_at
 */
class PageScratchNote extends Model
{
    protected $fillable = ['user_id', 'page_id', 'content'];

    public $timestamps = false;

    protected $casts = [
        'updated_at' => 'datetime',
    ];

    /**
     * Get the user that owns this scratch note.
     *
     * @return BelongsTo<User, $this>
     */
    public function user(): BelongsTo
    {
        return $this->belongsTo(User::class);
    }

    /**
     * Get the page this scratch note belongs to.
     *
     * @return BelongsTo<Page, $this>
     */
    public function page(): BelongsTo
    {
        return $this->belongsTo(Page::class);
    }
}
