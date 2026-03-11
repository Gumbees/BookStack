<?php

namespace BookStack\OAuth\Models;

use BookStack\Activity\Models\Loggable;
use BookStack\Users\Models\User;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Database\Eloquent\Relations\HasMany;

class OAuthClient extends Model implements Loggable
{
    protected $table = 'oauth_clients';

    protected $fillable = ['name', 'instance_approved'];

    protected $casts = [
        'instance_approved' => 'boolean',
    ];

    public function accessTokens(): HasMany
    {
        return $this->hasMany(OAuthAccessToken::class, 'client_id');
    }

    public function createdByUser(): BelongsTo
    {
        return $this->belongsTo(User::class, 'created_by');
    }

    public function getUrl(string $path = ''): string
    {
        return url('/settings/oauth-clients/' . $this->id . '/' . ltrim($path, '/'));
    }

    public function logDescriptor(): string
    {
        return "({$this->id}) {$this->name}";
    }
}
