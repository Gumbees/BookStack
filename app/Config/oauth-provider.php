<?php

/**
 * OAuth 2.0 Authorization Server configuration.
 *
 * When enabled, BookStack acts as an OAuth 2.0 provider so external
 * applications (MCP clients, IDE integrations, etc.) can authenticate
 * against the BookStack API using standard OAuth flows with PKCE.
 *
 * These values provide defaults. Admin panel settings (Settings > OAuth)
 * override these when configured.
 */

return [

    // Enable the OAuth 2.0 Authorization Server endpoints.
    // Overridden by admin setting: oauth.enabled
    'enabled' => env('OAUTH_PROVIDER_ENABLED', false),

    // Access token lifetime in seconds.
    // Default: 30 days. MCP clients may not reliably refresh tokens,
    // so a longer TTL reduces re-authentication frequency.
    // Overridden by admin setting: oauth.access_token_ttl_days (in days)
    'access_token_ttl' => env('OAUTH_ACCESS_TOKEN_TTL', 2592000),

    // Refresh token lifetime in seconds.
    // Default: 90 days.
    // Overridden by admin setting: oauth.refresh_token_ttl_days (in days)
    'refresh_token_ttl' => env('OAUTH_REFRESH_TOKEN_TTL', 7776000),

    // Allow dynamic client registration (RFC 7591) via POST /oauth/register.
    // Disabled by default. Enable only if clients need to self-register.
    // Overridden by admin setting: oauth.dynamic_registration
    'dynamic_registration' => env('OAUTH_DYNAMIC_REGISTRATION', false),

];
