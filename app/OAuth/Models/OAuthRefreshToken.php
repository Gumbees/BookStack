<?php

namespace BookStack\OAuth\Models;

use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

class OAuthRefreshToken extends Model
{
    protected $table = 'oauth_refresh_tokens';

    protected $fillable = [
        'token',
        'access_token_id',
        'expires_at',
    ];

    protected $casts = [
        'expires_at' => 'datetime',
    ];

    public function accessToken(): BelongsTo
    {
        return $this->belongsTo(OAuthAccessToken::class, 'access_token_id');
    }

    public function isExpired(): bool
    {
        return $this->expires_at->isPast();
    }
}
