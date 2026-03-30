<?php

namespace BookStack\OAuth\Controllers;

use BookStack\Http\Controller;
use BookStack\OAuth\OAuthService;
use BookStack\Permissions\Permission;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Http\Response;
use Illuminate\Support\Facades\Validator;

class OAuthController extends Controller
{
    public function __construct(
        protected OAuthService $oauthService,
    ) {
    }

    /**
     * OAuth 2.0 Authorization Server Metadata (RFC 8414).
     */
    public function metadata(Request $request): JsonResponse
    {
        $baseUrl = rtrim(config('app.url'), '/');

        return response()->json([
            'issuer' => $baseUrl,
            'authorization_endpoint' => $baseUrl . '/oauth/authorize',
            'token_endpoint' => $baseUrl . '/oauth/token',
            'registration_endpoint' => $baseUrl . '/oauth/register',
            'response_types_supported' => ['code'],
            'grant_types_supported' => ['authorization_code', 'refresh_token'],
            'code_challenge_methods_supported' => ['S256'],
            'token_endpoint_auth_methods_supported' => ['none', 'client_secret_post'],
        ]);
    }

    /**
     * OAuth 2.0 Protected Resource Metadata (RFC 9728).
     */
    public function resourceMetadata(Request $request): JsonResponse
    {
        $baseUrl = rtrim(config('app.url'), '/');

        return response()->json([
            'resource' => $baseUrl,
            'authorization_servers' => [$baseUrl],
            'bearer_methods_supported' => ['header'],
        ]);
    }

    /**
     * Dynamic Client Registration (RFC 7591).
     * Gated by OAUTH_DYNAMIC_REGISTRATION env setting (default: disabled).
     */
    public function register(Request $request): JsonResponse
    {
        // H-2: Gate dynamic client registration
        if (!config('oauth-provider.dynamic_registration', false)) {
            return response()->json([
                'error' => 'registration_not_supported',
                'error_description' => 'Dynamic client registration is not enabled on this server',
            ], 403);
        }

        // H-2: Input validation
        $data = $request->json()->all();

        $validator = Validator::make($data, [
            'client_name' => ['required', 'string', 'max:255'],
            'redirect_uris' => ['required', 'array', 'max:10'],
            'redirect_uris.*' => ['required', 'string', 'max:2000'],
        ]);

        if ($validator->fails()) {
            return response()->json([
                'error' => 'invalid_client_metadata',
                'error_description' => $validator->errors()->first(),
            ], 400);
        }

        // H-3: Validate redirect_uri schemes
        $redirectUris = $data['redirect_uris'];
        foreach ($redirectUris as $uri) {
            $uriError = $this->validateRedirectUriScheme($uri);
            if ($uriError) {
                return response()->json([
                    'error' => 'invalid_redirect_uri',
                    'error_description' => $uriError,
                ], 400);
            }
        }

        $clientName = $data['client_name'];
        $client = $this->oauthService->registerClient($clientName, $redirectUris);

        $now = now()->timestamp;

        $response = [
            'client_id' => $client->client_id,
            'client_id_issued_at' => $now,
            'token_endpoint_auth_method' => 'none',
            'grant_types' => ['authorization_code', 'refresh_token'],
            'response_types' => ['code'],
            'redirect_uris' => $redirectUris,
            'client_name' => $clientName,
        ];

        if (isset($data['scope'])) {
            $response['scope'] = $data['scope'];
        }

        return response()->json($response, 201);
    }

    /**
     * Authorization endpoint — show the consent page.
     */
    public function authorize(Request $request): Response
    {
        $responseType = $request->query('response_type');
        $clientId = $request->query('client_id');
        $redirectUri = $request->query('redirect_uri');
        $state = $request->query('state');
        $codeChallenge = $request->query('code_challenge');
        $codeChallengeMethod = $request->query('code_challenge_method');

        if ($responseType !== 'code') {
            return $this->oauthError('unsupported_response_type', 'Only response_type=code is supported');
        }

        if (!$clientId || !$redirectUri) {
            return $this->oauthError('invalid_request', 'client_id and redirect_uri are required');
        }

        if (!$codeChallenge) {
            return $this->oauthError('invalid_request', 'code_challenge is required (PKCE)');
        }

        // M-2: Reject unsupported code_challenge_method early
        if ($codeChallengeMethod !== null && $codeChallengeMethod !== 'S256') {
            return $this->oauthError('invalid_request', 'Only code_challenge_method=S256 is supported');
        }

        // M-1: Validate code_challenge format (base64url, exactly 43 chars per RFC 7636)
        if (!preg_match('/^[A-Za-z0-9\-._~]{43}$/', $codeChallenge)) {
            return $this->oauthError('invalid_request', 'code_challenge must be exactly 43 base64url characters');
        }

        // M-7: Limit state parameter size
        if ($state !== null && strlen($state) > 512) {
            return $this->oauthError('invalid_request', 'state parameter must not exceed 512 characters');
        }

        $client = $this->oauthService->findClient($clientId);
        if (!$client) {
            return $this->oauthError('invalid_client', 'Unknown client_id');
        }

        // C-1: Validate redirect_uri against the client's registered list
        $registeredUris = $client->redirect_uris ? json_decode($client->redirect_uris, true) : null;
        if (empty($registeredUris) || !in_array($redirectUri, $registeredUris, true)) {
            return $this->oauthError('invalid_request', 'redirect_uri does not match any registered URI for this client');
        }

        $user = auth()->user();

        // Check user has API access
        if (!$user->can(Permission::AccessApi)) {
            return $this->oauthError('access_denied', 'Your account does not have API access permission');
        }

        // Instance-approved clients skip the consent screen
        if ($this->oauthService->isClientInstanceApproved($client)) {
            $code = $this->oauthService->createAuthCode(
                $user,
                $client,
                $redirectUri,
                $codeChallenge,
                $codeChallengeMethod ?: 'S256',
            );

            $queryParams = ['code' => $code];
            if (!empty($state)) {
                $queryParams['state'] = $state;
            }

            $separator = str_contains($redirectUri, '?') ? '&' : '?';
            return response('', 302)->header('Location', $redirectUri . $separator . http_build_query($queryParams));
        }

        // Store the OAuth parameters in session for the consent form
        session()->put('oauth_authorize', [
            'client_id' => $clientId,
            'client_db_id' => $client->id,
            'client_name' => $client->name,
            'redirect_uri' => $redirectUri,
            'state' => $state,
            'code_challenge' => $codeChallenge,
            'code_challenge_method' => $codeChallengeMethod ?: 'S256',
        ]);

        return response(view('oauth.authorize', [
            'clientName' => $client->name ?: 'An application',
            'userName' => $user->name,
        ]));
    }

    /**
     * Handle consent form submission.
     */
    public function authorizeSubmit(Request $request): Response
    {
        $params = session()->pull('oauth_authorize');
        if (!$params) {
            return $this->oauthError('invalid_request', 'Authorization session expired');
        }

        $user = auth()->user();

        // Check user has API access
        if (!$user->can(Permission::AccessApi)) {
            return $this->oauthError('access_denied', 'Your account does not have API access permission');
        }

        // User denied — L-1: only include state if it was actually provided
        if ($request->input('action') === 'deny') {
            $separator = str_contains($params['redirect_uri'], '?') ? '&' : '?';
            $denyParams = [
                'error' => 'access_denied',
                'error_description' => 'The user denied the authorization request',
            ];
            if (!empty($params['state'])) {
                $denyParams['state'] = $params['state'];
            }
            $redirectUrl = $params['redirect_uri'] . $separator . http_build_query($denyParams);
            return response('', 302)->header('Location', $redirectUrl);
        }

        $client = $this->oauthService->findClient($params['client_id']);
        if (!$client) {
            return $this->oauthError('invalid_client', 'Client no longer exists');
        }

        // C-2: Re-validate redirect_uri against the client's registered list (session was validated
        // at authorize time, but defense-in-depth in case client was updated between the two requests)
        $registeredUris = $client->redirect_uris ? json_decode($client->redirect_uris, true) : null;
        if (empty($registeredUris) || !in_array($params['redirect_uri'], $registeredUris, true)) {
            return $this->oauthError('invalid_request', 'redirect_uri does not match any registered URI for this client');
        }

        $code = $this->oauthService->createAuthCode(
            $user,
            $client,
            $params['redirect_uri'],
            $params['code_challenge'],
            $params['code_challenge_method'],
        );

        $queryParams = ['code' => $code];
        if (!empty($params['state'])) {
            $queryParams['state'] = $params['state'];
        }

        $separator = str_contains($params['redirect_uri'], '?') ? '&' : '?';
        $redirectUrl = $params['redirect_uri'] . $separator . http_build_query($queryParams);

        return response('', 302)->header('Location', $redirectUrl);
    }

    /**
     * Token endpoint — exchange auth code or refresh token.
     */
    public function token(Request $request): JsonResponse
    {
        $grantType = $request->input('grant_type');

        return match ($grantType) {
            'authorization_code' => $this->tokenAuthorizationCode($request),
            'refresh_token' => $this->tokenRefresh($request),
            default => response()->json([
                'error' => 'unsupported_grant_type',
                'error_description' => 'Supported: authorization_code, refresh_token',
            ], 400),
        };
    }

    protected function tokenAuthorizationCode(Request $request): JsonResponse
    {
        $code = $request->input('code');
        $clientId = $request->input('client_id');
        $redirectUri = $request->input('redirect_uri');
        $codeVerifier = $request->input('code_verifier');

        if (!$code || !$clientId || !$redirectUri) {
            return response()->json([
                'error' => 'invalid_request',
                'error_description' => 'code, client_id, and redirect_uri are required',
            ], 400);
        }

        $tokens = $this->oauthService->exchangeAuthCode($code, $clientId, $redirectUri, $codeVerifier);

        if (!$tokens) {
            return response()->json([
                'error' => 'invalid_grant',
                'error_description' => 'Invalid or expired authorization code, or PKCE verification failed',
            ], 400);
        }

        return response()->json([
            'access_token' => $tokens['access_token'],
            'token_type' => 'bearer',
            'expires_in' => $tokens['expires_in'],
            'refresh_token' => $tokens['refresh_token'],
        ])->header('Cache-Control', 'no-store')->header('Pragma', 'no-cache');
    }

    protected function tokenRefresh(Request $request): JsonResponse
    {
        $refreshToken = $request->input('refresh_token');

        if (!$refreshToken) {
            return response()->json([
                'error' => 'invalid_request',
                'error_description' => 'refresh_token is required',
            ], 400);
        }

        $tokens = $this->oauthService->refreshAccessToken($refreshToken);

        if (!$tokens) {
            return response()->json([
                'error' => 'invalid_grant',
                'error_description' => 'Invalid or expired refresh token',
            ], 400);
        }

        return response()->json([
            'access_token' => $tokens['access_token'],
            'token_type' => 'bearer',
            'expires_in' => $tokens['expires_in'],
            'refresh_token' => $tokens['refresh_token'],
        ])->header('Cache-Control', 'no-store')->header('Pragma', 'no-cache');
    }

    /**
     * Validate that a redirect URI has an acceptable scheme.
     * Allows https:// for all hosts, and http:// only for localhost / 127.0.0.1 (RFC 8252).
     * Returns an error string on failure, or null on success.
     */
    protected function validateRedirectUriScheme(string $uri): ?string
    {
        $parsed = parse_url($uri);

        if ($parsed === false || empty($parsed['scheme']) || empty($parsed['host'])) {
            return "'{$uri}' is not a valid URL";
        }

        $scheme = strtolower($parsed['scheme']);
        $host = strtolower($parsed['host']);

        if (in_array($scheme, ['javascript', 'data', 'vbscript'], true)) {
            return "URI scheme '{$scheme}' is not permitted";
        }

        if ($scheme === 'https') {
            return null;
        }

        if ($scheme === 'http' && in_array($host, ['localhost', '127.0.0.1'], true)) {
            return null;
        }

        return "redirect_uri must use https (http is only allowed for localhost/127.0.0.1)";
    }

    protected function oauthError(string $error, string $description, int $status = 400): Response
    {
        return response(json_encode([
            'error' => $error,
            'error_description' => $description,
        ]), $status)->header('Content-Type', 'application/json');
    }
}
