<?php

namespace BookStack\OAuth\Controllers;

use BookStack\Activity\ActivityType;
use BookStack\Http\Controller;
use BookStack\OAuth\Models\OAuthClient;
use BookStack\OAuth\OAuthService;
use BookStack\Permissions\Permission;
use Illuminate\Http\Request;

class OAuthClientController extends Controller
{
    public function __construct(
        protected OAuthService $oauthService,
    ) {
        $this->middleware([
            Permission::SettingsManage->middleware(),
        ]);
    }

    /**
     * List all registered OAuth clients.
     */
    public function index()
    {
        $clients = $this->oauthService->getAllClients();

        $this->setPageTitle(trans('settings.oauth_clients'));

        return view('settings.oauth-clients.index', [
            'clients' => $clients,
        ]);
    }

    /**
     * Show a single OAuth client with its authorizations.
     */
    public function show(string $id)
    {
        $client = OAuthClient::query()
            ->withCount(['accessTokens' => fn ($q) => $q->where('expires_at', '>', now())])
            ->with('createdByUser')
            ->findOrFail($id);

        // M-4: Query authorizations directly for this client instead of loading all
        $authorizations = $this->oauthService->getActiveAuthorizationsForClient($client);

        $this->setPageTitle(trans('settings.oauth_client') . ' - ' . $client->name);

        return view('settings.oauth-clients.show', [
            'client' => $client,
            'authorizations' => $authorizations,
        ]);
    }

    /**
     * Update an OAuth client (name, instance_approved).
     */
    public function update(Request $request, string $id)
    {
        $validated = $this->validate($request, [
            'name' => ['required', 'max:150'],
            'instance_approved' => ['required'],
        ]);

        $client = OAuthClient::query()->findOrFail($id);
        $client->name = $validated['name'];
        $client->instance_approved = $validated['instance_approved'] === 'true';
        $client->save();

        $this->logActivity(ActivityType::OAUTH_CLIENT_UPDATE, $client);
        $this->showSuccessNotification(trans('settings.oauth_client_updated'));

        return redirect($client->getUrl());
    }

    /**
     * Show the delete confirmation page.
     */
    public function delete(string $id)
    {
        $client = OAuthClient::query()->findOrFail($id);

        $this->setPageTitle(trans('settings.oauth_client_delete'));

        return view('settings.oauth-clients.delete', [
            'client' => $client,
        ]);
    }

    /**
     * Destroy an OAuth client and all its tokens.
     */
    public function destroy(string $id)
    {
        $client = OAuthClient::query()->findOrFail($id);

        $this->logActivity(ActivityType::OAUTH_CLIENT_DELETE, $client);
        $this->oauthService->revokeClient($client);

        $this->showSuccessNotification(trans('settings.oauth_client_deleted'));

        return redirect('/settings/oauth-clients');
    }

    /**
     * Revoke a specific user's authorization for a client.
     */
    public function revokeAuthorization(Request $request, string $id)
    {
        $client = OAuthClient::query()->findOrFail($id);
        $userId = $request->input('user_id');

        $user = \BookStack\Users\Models\User::query()->findOrFail($userId);
        $this->oauthService->revokeUserAuthorizationForClient($user, $client);

        $this->logActivity(ActivityType::OAUTH_AUTHORIZATION_REVOKE, $client);
        $this->showSuccessNotification(trans('settings.oauth_authorization_revoked'));

        return redirect($client->getUrl());
    }
}
