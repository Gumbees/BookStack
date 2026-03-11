<?php

/**
 * Routes for the BookStack OAuth 2.0 Authorization Server.
 *
 * These endpoints allow external applications (MCP clients, IDE integrations, etc.)
 * to authenticate against the BookStack API using standard OAuth 2.0 with PKCE.
 *
 * Enable via: OAUTH_PROVIDER_ENABLED=true
 */

use BookStack\OAuth\Controllers\OAuthController;
use Illuminate\Support\Facades\Route;

// Discovery endpoints — no auth required
Route::get('/.well-known/oauth-authorization-server', [OAuthController::class, 'metadata']);
Route::get('/.well-known/oauth-protected-resource', [OAuthController::class, 'resourceMetadata']);

// Dynamic client registration — no auth required
Route::post('/oauth/register', [OAuthController::class, 'register']);

// Authorization — requires authenticated user (web session)
Route::middleware('auth')->group(function () {
    Route::get('/oauth/authorize', [OAuthController::class, 'authorize']);
    Route::post('/oauth/authorize', [OAuthController::class, 'authorizeSubmit']);
});

// Token exchange — no auth required (uses auth codes / refresh tokens)
Route::post('/oauth/token', [OAuthController::class, 'token']);
