<?php

namespace BookStack\Collaboration;

use BookStack\Users\Models\User;
use InvalidArgumentException;

class CollabTokenService
{
    /**
     * Generate a short-lived JWT for the given user and page.
     * The token is signed with HMAC-SHA256 using the shared secret from config.
     *
     * @throws InvalidArgumentException when the jwt secret is not configured.
     */
    public function generateToken(User $user, int $pageId): string
    {
        $secret = config('collab.jwt_secret');
        if (empty($secret)) {
            throw new InvalidArgumentException('COLLAB_JWT_SECRET is not configured.');
        }

        $header = $this->base64UrlEncode(json_encode([
            'alg' => 'HS256',
            'typ' => 'JWT',
        ]));

        $payload = $this->base64UrlEncode(json_encode([
            'user_id'   => $user->id,
            'user_name' => $user->name,
            'page_id'   => $pageId,
            'exp'       => time() + 300, // 5-minute expiry
        ]));

        $signature = $this->base64UrlEncode(
            hash_hmac('sha256', "{$header}.{$payload}", $secret, true)
        );

        return "{$header}.{$payload}.{$signature}";
    }

    private function base64UrlEncode(string $data): string
    {
        return rtrim(strtr(base64_encode($data), '+/', '-_'), '=');
    }
}
