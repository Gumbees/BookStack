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
    const VALID_SCOPES = ['read', 'write', 'admin'];
    const DEFAULT_SCOPES = 'read write';

    /**
     * Check if the OAuth provider feature is enabled.
     */
    public static function enabled(): bool
    {
        $setting = setting('oauth.enabled');
        if ($setting !== null && $setting !== '') {
            return $setting === 'true' || $setting === true;
        }

        return (bool) config('oauth-provider.enabled', false);
    }

    /**
     * Check if dynamic client registration is enabled.
     */
    public static function dynamicRegistrationEnabled(): bool
    {
        $setting = setting('oauth.dynamic_registration');
        if ($setting !== null && $setting !== '') {
            return $setting === 'true' || $setting === true;
        }

        return (bool) config('oauth-provider.dynamic_registration', false);
    }

    /**
     * Get the access token TTL in seconds.
     */
    public static function accessTokenTtl(): int
    {
        $days = setting('oauth.access_token_ttl_days');
        if ($days !== null && $days !== '') {
            return (int) $days * 86400;
        }

        return (int) config('oauth-provider.access_token_ttl', 2592000);
    }

    /**
     * Get the refresh token TTL in seconds.
     */
    public static function refreshTokenTtl(): int
    {
        $days = setting('oauth.refresh_token_ttl_days');
        if ($days !== null && $days !== '') {
            return (int) $days * 86400;
        }

        return (int) config('oauth-provider.refresh_token_ttl', 7776000);
    }

    /**
     * Check if SSO lifetime honoring is enabled.
     */
    public static function honorSsoLifetime(): bool
    {
        $setting = setting('oauth.honor_sso_lifetime');
        if ($setting !== null && $setting !== '') {
            return $setting === 'true' || $setting === true;
        }

        return true;
    }

    /**
     * Validate a space-separated scope string.
     * Returns the normalized scope string if valid, or null if any scope is invalid.
     */
    public function validateScopes(?string $scopeString): ?string
    {
        if ($scopeString === null || trim($scopeString) === '') {
            return self::DEFAULT_SCOPES;
        }

        $scopes = array_unique(array_filter(explode(' ', trim($scopeString))));

        foreach ($scopes as $scope) {
            if (!in_array($scope, self::VALID_SCOPES, true)) {
                return null;
            }
        }

        return implode(' ', $scopes);
    }

    /**
     * Register a new OAuth client.
     * If $confidential is true, generates a client secret.
     *
     * @return array{client: OAuthClient, secret: string|null}
     */
    public function registerClient(
        string $name,
        ?array $redirectUris = null,
        ?int $createdBy = null,
        bool $confidential = false,
    ): array {
        $client = new OAuthClient();
        $client->client_id = Str::random(32);
        $client->name = $name;
        $client->redirect_uris = $redirectUris ? json_encode($redirectUris) : null;
        $client->created_by = $createdBy;
        $client->confidential = $confidential;

        $plainSecret = null;

        if ($confidential) {
            $plainSecret = Str::random(64);
            $client->client_secret = hash('sha256', $plainSecret);
        }

        $client->save();

        return [
            'client' => $client,
            'secret' => $plainSecret,
        ];
    }

    /**
     * Validate a client secret against the stored hash.
     */
    public function validateClientSecret(OAuthClient $client, ?string $secret): bool
    {
        if (!$client->confidential || !$client->client_secret) {
            return true;
        }

        if (!$secret) {
            return false;
        }

        return hash_equals($client->client_secret, hash('sha256', $secret));
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
        ?string $scopes = null,
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
        $authCode->scopes = $scopes;
        $authCode->expires_at = Carbon::now()->addMinutes(5);
        $authCode->save();

        return $plainCode;
    }

    /**
     * Exchange an authorization code for tokens.
     * Returns null if the code is invalid or expired.
     *
     * @return array{access_token: string, refresh_token: string, expires_in: int, scope: string}|null
     */
    public function exchangeAuthCode(
        string $code,
        string $clientId,
        string $redirectUri,
        ?string $codeVerifier = null,
        ?string $clientSecret = null,
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

        // Validate client secret for confidential clients
        if (!$this->validateClientSecret($client, $clientSecret)) {
            return null;
        }

        // Validate redirect URI against stored value from authorization
        if ($authCode->redirect_uri !== $redirectUri) {
            return null;
        }

        // M-3: Defense-in-depth — also validate redirect_uri against the client's registered list
        $registeredUris = $client->redirect_uris ? json_decode($client->redirect_uris, true) : null;
        if (!empty($registeredUris) && !in_array($redirectUri, $registeredUris, true)) {
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

        $scopes = $authCode->scopes ?: self::DEFAULT_SCOPES;

        return $this->issueTokens($user, $client, $scopes);
    }

    /**
     * Refresh an access token using a refresh token.
     *
     * @return array{access_token: string, refresh_token: string, expires_in: int, scope: string}|null
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

        // Preserve scopes from the old token
        $scopes = $oldAccess->scopes ?: self::DEFAULT_SCOPES;

        // Delete old tokens (rotation)
        $storedRefresh->delete();
        $oldAccess->delete();

        return $this->issueTokens($user, $oldAccess->client, $scopes);
    }

    /**
     * Look up a user by their OAuth access token.
     * Updates last_used_at timestamp on successful lookup.
     * Stores token scopes on the request for downstream access.
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

        $accessToken->last_used_at = Carbon::now();
        $accessToken->save();

        // Store scopes on the request for downstream middleware/authorization
        if (app()->bound('request')) {
            request()->attributes->set('oauth_scopes', $accessToken->scopes ?: self::DEFAULT_SCOPES);
        }

        return User::query()->find($accessToken->user_id);
    }

    /**
     * Issue a new access token + refresh token pair.
     *
     * @return array{access_token: string, refresh_token: string, expires_in: int, scope: string}
     */
    protected function issueTokens(User $user, OAuthClient $client, string $scopes = self::DEFAULT_SCOPES): array
    {
        $accessTtl = self::accessTokenTtl();
        $refreshTtl = self::refreshTokenTtl();

        // Honor SSO session lifetime when upstream auth is OIDC or SAML
        if (self::honorSsoLifetime()) {
            $authMethod = config('auth.method');
            if (in_array($authMethod, ['oidc', 'saml2'], true)) {
                $sessionLifetimeSeconds = (int) config('session.lifetime', 120) * 60;
                if ($sessionLifetimeSeconds > 0) {
                    $accessTtl = min($accessTtl, $sessionLifetimeSeconds);
                }
            }
        }

        $plainAccess = Str::random(64);
        $plainRefresh = Str::random(64);

        $accessToken = new OAuthAccessToken();
        $accessToken->token = hash('sha256', $plainAccess);
        $accessToken->user_id = $user->id;
        $accessToken->client_id = $client->id;
        $accessToken->scopes = $scopes;
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
            'scope' => $scopes,
        ];
    }

    /**
     * Revoke an OAuth client and all its tokens.
     */
    public function revokeClient(OAuthClient $client): void
    {
        // Delete all access tokens (cascades to refresh tokens via DB foreign key)
        $client->accessTokens()->delete();

        // Delete any pending auth codes
        OAuthAuthCode::query()->where('client_id', $client->id)->delete();

        $client->delete();
    }

    /**
     * Revoke a specific user's authorization for a client.
     * Deletes all access/refresh tokens for that user+client pair.
     */
    public function revokeUserAuthorizationForClient(User $user, OAuthClient $client): void
    {
        $tokenIds = OAuthAccessToken::query()
            ->where('client_id', $client->id)
            ->where('user_id', $user->id)
            ->pluck('id');

        OAuthRefreshToken::query()->whereIn('access_token_id', $tokenIds)->delete();
        OAuthAccessToken::query()->whereIn('id', $tokenIds)->delete();
        OAuthAuthCode::query()
            ->where('client_id', $client->id)
            ->where('user_id', $user->id)
            ->delete();
    }

    /**
     * Get all active OAuth authorizations for a specific user.
     * Returns clients with their latest token activity.
     */
    public function getActiveAuthorizationsForUser(User $user): \Illuminate\Support\Collection
    {
        return OAuthAccessToken::query()
            ->where('user_id', $user->id)
            ->where('expires_at', '>', Carbon::now())
            ->with('client')
            ->get()
            ->groupBy('client_id')
            ->map(function ($tokens) {
                $latest = $tokens->sortByDesc('created_at')->first();
                return (object) [
                    'client' => $latest->client,
                    'token_count' => $tokens->count(),
                    'last_used_at' => $tokens->max('last_used_at'),
                    'created_at' => $tokens->min('created_at'),
                ];
            })
            ->values();
    }

    /**
     * Get active authorizations for a specific client (admin view).
     * M-4: Queries directly with a where clause rather than loading all authorizations.
     */
    public function getActiveAuthorizationsForClient(OAuthClient $client): \Illuminate\Support\Collection
    {
        return OAuthAccessToken::query()
            ->where('client_id', $client->id)
            ->where('expires_at', '>', Carbon::now())
            ->with('user')
            ->get()
            ->groupBy('user_id')
            ->map(function ($tokens) use ($client) {
                $latest = $tokens->sortByDesc('created_at')->first();
                return (object) [
                    'client' => $client,
                    'user' => $latest->user,
                    'token_count' => $tokens->count(),
                    'last_used_at' => $tokens->max('last_used_at'),
                    'created_at' => $tokens->min('created_at'),
                ];
            })
            ->values();
    }

    /**
     * Get all active authorizations across all users (admin view).
     * Returns a collection of objects with client, user, and token info.
     */
    public function getAllActiveAuthorizations(): \Illuminate\Support\Collection
    {
        return OAuthAccessToken::query()
            ->where('expires_at', '>', Carbon::now())
            ->with(['client', 'user'])
            ->get()
            ->groupBy(fn ($t) => $t->client_id . ':' . $t->user_id)
            ->map(function ($tokens) {
                $latest = $tokens->sortByDesc('created_at')->first();
                return (object) [
                    'client' => $latest->client,
                    'user' => $latest->user,
                    'token_count' => $tokens->count(),
                    'last_used_at' => $tokens->max('last_used_at'),
                    'created_at' => $tokens->min('created_at'),
                ];
            })
            ->values();
    }

    /**
     * Check if a client is instance-approved (users skip consent screen).
     */
    public function isClientInstanceApproved(OAuthClient $client): bool
    {
        return (bool) $client->instance_approved;
    }

    /**
     * Get all registered OAuth clients with related data.
     * M-5: withCount scoped to exclude expired tokens.
     */
    public function getAllClients(): \Illuminate\Support\Collection
    {
        return OAuthClient::query()
            ->withCount(['accessTokens' => fn ($q) => $q->where('expires_at', '>', now())])
            ->with('createdByUser')
            ->orderBy('created_at', 'desc')
            ->get();
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
