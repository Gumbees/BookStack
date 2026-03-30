<?php

namespace BookStack\Activity\Models;

use Illuminate\Database\Eloquent\Model;
use Illuminate\Support\Carbon;

/**
 * @property int    $id
 * @property int    $scratch_note_id
 * @property int    $user_id
 * @property Carbon $created_at
 */
class ScratchNoteMentionHistory extends Model
{
    public $timestamps = false;

    protected $table = 'scratch_note_mention_history';

    protected $fillable = ['scratch_note_id', 'user_id', 'created_at'];
}
