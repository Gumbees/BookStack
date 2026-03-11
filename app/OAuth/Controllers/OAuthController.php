<?php

namespace BookStack\OAuth\Controllers;

use BookStack\Http\Controller;
use BookStack\OAuth\OAuthService;
use BookStack\Permissions\Permission;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Http\Response;

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
     */
    public function register(Request $request): JsonResponse
    {
        $data = $request->json()->all();
        $clientName = $data['client_name'] ?? 'Unnamed Client';
        $redirectUris = $data['redirect_uris'] ?? null;

        $client = $this->oauthService->registerClient($clientName, $redirectUris);

        $now = now()->timestamp;

        $response = [
            'client_id' => $client->client_id,
            'client_id_issued_at' => $now,
            'token_endpoint_auth_method' => 'none',
            'grant_types' => ['authorization_code', 'refresh_token'],
            'response_types' => ['code'],
        ];

        if (isset($data['redirect_uris'])) {
            $response['redirect_uris'] = $data['redirect_uris'];
        }
        if (isset($data['client_name'])) {
            $response['client_name'] = $data['client_name'];
        }
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

        $client = $this->oauthService->findClient($clientId);
        if (!$client) {
            return $this->oauthError('invalid_client', 'Unknown client_id');
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

        // User denied
        if ($request->input('action') === 'deny') {
            $separator = str_contains($params['redirect_uri'], '?') ? '&' : '?';
            $redirectUrl = $params['redirect_uri'] . $separator . http_build_query([
                'error' => 'access_denied',
                'error_description' => 'The user denied the authorization request',
                'state' => $params['state'] ?? '',
            ]);
            return response('', 302)->header('Location', $redirectUrl);
        }

        $client = $this->oauthService->findClient($params['client_id']);
        if (!$client) {
            return $this->oauthError('invalid_client', 'Client no longer exists');
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

    protected function oauthError(string $error, string $description, int $status = 400): Response
    {
        return response(json_encode([
            'error' => $error,
            'error_description' => $description,
        ]), $status)->header('Content-Type', 'application/json');
    }
}
