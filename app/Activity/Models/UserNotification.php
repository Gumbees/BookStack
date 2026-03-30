<?php

namespace BookStack\Activity\Models;

use BookStack\App\Model;
use BookStack\Users\Models\User;
use Carbon\Carbon;
use Illuminate\Database\Eloquent\Builder;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

/**
 * @property int    $id
 * @property int    $user_id
 * @property string $type
 * @property array  $data
 * @property ?Carbon $read_at
 * @property ?Carbon $created_at
 */
class UserNotification extends Model
{
    public $timestamps = false;

    protected $table = 'user_notifications';

    protected $fillable = ['user_id', 'type', 'data', 'read_at', 'created_at'];

    protected $casts = [
        'data'       => 'array',
        'read_at'    => 'datetime',
        'created_at' => 'datetime',
    ];

    /**
     * Get the user this notification belongs to.
     *
     * @return BelongsTo<User, $this>
     */
    public function user(): BelongsTo
    {
        return $this->belongsTo(User::class);
    }

    /**
     * Scope to unread notifications.
     */
    public function scopeUnread(Builder $query): Builder
    {
        return $query->whereNull('read_at');
    }

    /**
     * Scope to notifications for a specific user.
     */
    public function scopeForUser(Builder $query, int $userId): Builder
    {
        return $query->where('user_id', $userId);
    }

    /**
     * Mark this notification as read.
     */
    public function markAsRead(): void
    {
        if (is_null($this->read_at)) {
            $this->read_at = now();
            $this->save();
        }
    }
}
