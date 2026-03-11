<?php

namespace BookStack\OAuth\Models;

use Illuminate\Database\Eloquent\Model;

class OAuthAuthCode extends Model
{
    protected $table = 'oauth_auth_codes';

    protected $casts = [
        'expires_at' => 'datetime',
    ];
}
