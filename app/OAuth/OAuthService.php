<?php

namespace BookStack\OAuth;

use BookStack\OAuth\Models\OAuthAccessToken;
use BookStack\OAuth\Models\OAuthAuthCode;
use BookStack\OAuth\Models\OAuthClient;
use BookStack\OAuth\Models\OAuthRefreshToken;
use BookStack\Users\Models\User;
use Illuminate\Support\Carbon;
use Illuminate\Support\Str;

class OAuthService
{
    /**
     * Check if the OAuth provider feature is enabled.
     */
    public static function enabled(): bool
    {
        return (bool) config('oauth-provider.enabled', false);
    }

    /**
     * Get the access token TTL in seconds.
     */
    public static function accessTokenTtl(): int
    {
        return (int) config('oauth-provider.access_token_ttl', 2592000);
    }

    /**
     * Get the refresh token TTL in seconds.
     */
    public static function refreshTokenTtl(): int
    {
        return (int) config('oauth-provider.refresh_token_ttl', 7776000);
    }

    /**
     * Register a new dynamic OAuth client.
     */
    public function registerClient(string $name, ?array $redirectUris = null): OAuthClient
    {
        $client = new OAuthClient();
        $client->client_id = Str::random(32);
        $client->name = $name;
        $client->redirect_uris = $redirectUris ? json_encode($redirectUris) : null;
        $client->save();

        return $client;
    }

    /**
     * Find a client by its public client_id string.
     */
    public function findClient(string $clientId): ?OAuthClient
    {
        return OAuthClient::query()->where('client_id', $clientId)->first();
    }

    /**
     * Create an authorization code for the given user and client.
     */
    public function createAuthCode(
        User $user,
        OAuthClient $client,
        string $redirectUri,
        ?string $codeChallenge = null,
        ?string $codeChallengeMethod = null,
    ): string {
        // Clean up expired codes for this user/client
        OAuthAuthCode::query()
            ->where('client_id', $client->id)
            ->where('user_id', $user->id)
            ->where('expires_at', '<', Carbon::now())
            ->delete();

        $plainCode = Str::random(64);

        $authCode = new OAuthAuthCode();
        $authCode->code = hash('sha256', $plainCode);
        $authCode->client_id = $client->id;
        $authCode->user_id = $user->id;
        $authCode->redirect_uri = $redirectUri;
        $authCode->code_challenge = $codeChallenge;
        $authCode->code_challenge_method = $codeChallengeMethod;
        $authCode->expires_at = Carbon::now()->addMinutes(5);
        $authCode->save();

        return $plainCode;
    }

    /**
     * Exchange an authorization code for tokens.
     * Returns null if the code is invalid or expired.
     *
     * @return array{access_token: string, refresh_token: string, expires_in: int}|null
     */
    public function exchangeAuthCode(
        string $code,
        string $clientId,
        string $redirectUri,
        ?string $codeVerifier = null,
    ): ?array {
        $codeHash = hash('sha256', $code);

        $authCode = OAuthAuthCode::query()
            ->where('code', $codeHash)
            ->where('expires_at', '>', Carbon::now())
            ->first();

        if (!$authCode) {
            return null;
        }

        // Always delete the auth code (single use)
        $authCode->delete();

        // Validate client
        $client = OAuthClient::query()->find($authCode->client_id);
        if (!$client || $client->client_id !== $clientId) {
            return null;
        }

        // Validate redirect URI
        if ($authCode->redirect_uri !== $redirectUri) {
            return null;
        }

        // Validate PKCE
        if ($authCode->code_challenge) {
            if (!$codeVerifier) {
                return null;
            }

            $method = $authCode->code_challenge_method ?: 'S256';
            if ($method !== 'S256') {
                return null;
            }

            $computed = rtrim(strtr(base64_encode(hash('sha256', $codeVerifier, true)), '+/', '-_'), '=');
            if (!hash_equals($authCode->code_challenge, $computed)) {
                return null;
            }
        }

        // Load user and verify they still have API access
        $user = User::query()->find($authCode->user_id);
        if (!$user) {
            return null;
        }

        return $this->issueTokens($user, $client);
    }

    /**
     * Refresh an access token using a refresh token.
     *
     * @return array{access_token: string, refresh_token: string, expires_in: int}|null
     */
    public function refreshAccessToken(string $refreshToken): ?array
    {
        $tokenHash = hash('sha256', $refreshToken);

        $storedRefresh = OAuthRefreshToken::query()
            ->where('token', $tokenHash)
            ->where('expires_at', '>', Carbon::now())
            ->first();

        if (!$storedRefresh) {
            return null;
        }

        $oldAccess = OAuthAccessToken::query()
            ->with('client')
            ->find($storedRefresh->access_token_id);

        if (!$oldAccess || !$oldAccess->client) {
            $storedRefresh->delete();
            return null;
        }

        $user = User::query()->find($oldAccess->user_id);
        if (!$user) {
            $storedRefresh->delete();
            $oldAccess->delete();
            return null;
        }

        // Delete old tokens (rotation)
        $storedRefresh->delete();
        $oldAccess->delete();

        return $this->issueTokens($user, $oldAccess->client);
    }

    /**
     * Look up a user by their OAuth access token.
     */
    public function findUserByAccessToken(string $token): ?User
    {
        $tokenHash = hash('sha256', $token);

        $accessToken = OAuthAccessToken::query()
            ->where('token', $tokenHash)
            ->where('expires_at', '>', Carbon::now())
            ->first();

        if (!$accessToken) {
            return null;
        }

        return User::query()->find($accessToken->user_id);
    }

    /**
     * Issue a new access token + refresh token pair.
     *
     * @return array{access_token: string, refresh_token: string, expires_in: int}
     */
    protected function issueTokens(User $user, OAuthClient $client): array
    {
        $accessTtl = self::accessTokenTtl();
        $refreshTtl = self::refreshTokenTtl();

        $plainAccess = Str::random(64);
        $plainRefresh = Str::random(64);

        $accessToken = new OAuthAccessToken();
        $accessToken->token = hash('sha256', $plainAccess);
        $accessToken->user_id = $user->id;
        $accessToken->client_id = $client->id;
        $accessToken->expires_at = Carbon::now()->addSeconds($accessTtl);
        $accessToken->save();

        $refreshToken = new OAuthRefreshToken();
        $refreshToken->token = hash('sha256', $plainRefresh);
        $refreshToken->access_token_id = $accessToken->id;
        $refreshToken->expires_at = Carbon::now()->addSeconds($refreshTtl);
        $refreshToken->save();

        return [
            'access_token' => $plainAccess,
            'refresh_token' => $plainRefresh,
            'expires_in' => $accessTtl,
        ];
    }

    /**
     * Clean up expired tokens and codes.
     */
    public function cleanupExpired(): void
    {
        $now = Carbon::now();

        OAuthAuthCode::query()->where('expires_at', '<', $now)->delete();

        // Delete expired access tokens (cascades to refresh tokens)
        OAuthAccessToken::query()->where('expires_at', '<', $now)->delete();

        // Delete orphaned refresh tokens
        OAuthRefreshToken::query()->where('expires_at', '<', $now)->delete();
    }
}
