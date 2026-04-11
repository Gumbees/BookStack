<?php

namespace BookStack\OAuth\Models;

use Illuminate\Database\Eloquent\Model;

class OAuthAuthCode extends Model
{
    protected $table = 'oauth_auth_codes';

    protected $fillable = [
        'code',
        'client_id',
        'user_id',
        'redirect_uri',
        'code_challenge',
        'code_challenge_method',
        'scopes',
        'expires_at',
    ];

    protected $casts = [
        'expires_at' => 'datetime',
    ];
}
