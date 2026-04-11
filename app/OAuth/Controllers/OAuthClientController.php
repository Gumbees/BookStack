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
     * Show the form to create a new OAuth client.
     */
    public function create()
    {
        $this->setPageTitle(trans('settings.oauth_client_create'));

        return view('settings.oauth-clients.create');
    }

    /**
     * Store a newly created OAuth client.
     */
    public function store(Request $request)
    {
        $validated = $this->validate($request, [
            'name' => ['required', 'string', 'max:150'],
            'redirect_uris' => ['required', 'string'],
            'confidential' => ['nullable'],
            'instance_approved' => ['nullable'],
        ]);

        // Parse redirect URIs (one per line, handle \r\n line endings)
        $rawUris = array_values(array_filter(array_map('trim', preg_split('/\r?\n/', $validated['redirect_uris']))));
        if (empty($rawUris)) {
            return redirect()->back()->withInput()->withErrors(['redirect_uris' => trans('settings.oauth_client_create_redirect_required')]);
        }

        // Validate each URI scheme
        foreach ($rawUris as $uri) {
            $uriError = $this->validateRedirectUriScheme($uri);
            if ($uriError) {
                return redirect()->back()->withInput()->withErrors(['redirect_uris' => $uriError]);
            }
        }

        $confidential = ($request->input('confidential') === 'true');
        $instanceApproved = ($request->input('instance_approved') === 'true');

        $result = $this->oauthService->registerClient(
            $validated['name'],
            $rawUris,
            user()->id,
            $confidential,
        );

        $client = $result['client'];
        $client->instance_approved = $instanceApproved;
        $client->save();

        $this->logActivity(ActivityType::OAUTH_CLIENT_CREATE, $client);
        $this->showSuccessNotification(trans('settings.oauth_client_created'));

        // Flash the secret so it can be shown once on the show page
        if ($result['secret']) {
            session()->flash('oauth_client_secret', $result['secret']);
        }

        return redirect($client->getUrl());
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
            'secret' => session('oauth_client_secret'),
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

    /**
     * Validate that a redirect URI has an acceptable scheme.
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

        return trans('settings.oauth_client_create_redirect_https');
    }
}
