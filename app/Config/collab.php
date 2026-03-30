<?php

return [
    'enabled' => env('COLLAB_ENABLED', false),
    'server_url' => env('COLLAB_SERVER_URL', 'ws://localhost:7700'),
    'jwt_secret' => env('COLLAB_JWT_SECRET', ''),
];
