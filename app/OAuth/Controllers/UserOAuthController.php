<?php

namespace BookStack\OAuth\Controllers;

use BookStack\Activity\ActivityType;
use BookStack\Http\Controller;
use BookStack\OAuth\Models\OAuthClient;
use BookStack\OAuth\OAuthService;
use Illuminate\Http\Request;

class UserOAuthController extends Controller
{
    public function __construct(
        protected OAuthService $oauthService,
    ) {
    }

    /**
     * Revoke the current user's authorization for a specific OAuth client.
     */
    public function revokeAuthorization(Request $request, string $clientId)
    {
        $client = OAuthClient::query()->findOrFail($clientId);

        $this->oauthService->revokeUserAuthorizationForClient(user(), $client);

        $this->logActivity(ActivityType::OAUTH_AUTHORIZATION_REVOKE, $client);
        $this->showSuccessNotification(trans('settings.oauth_authorization_revoked'));

        return redirect('/my-account/auth');
    }
}
