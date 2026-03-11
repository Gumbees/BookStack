<section class="card content-wrap auto-height" id="oauth_authorizations">
    <div class="flex-container-row wrap justify-space-between items-center mb-s">
        <h2 class="list-heading">{{ trans('settings.oauth_authorized_apps') }}</h2>
    </div>
    <p class="text-small text-muted">{{ trans('settings.oauth_authorized_apps_desc') }}</p>
    @if (count($oauthAuthorizations) > 0)
        <div class="item-list my-m">
            @foreach($oauthAuthorizations as $auth)
                <div class="item-list-row flex-container-row items-center wrap py-xs gap-x-m">
                    <div class="flex px-m py-xs min-width-m">
                        <strong>{{ $auth->client->name }}</strong>
                        <br>
                        <span class="small text-muted italic">{{ trans('settings.oauth_authorized_since') }}: {{ \Illuminate\Support\Carbon::parse($auth->created_at)->format('Y-m-d') }}</span>
                    </div>
                    <div class="flex flex-container-row items-center min-width-m">
                        <div class="flex px-m py-xs text-muted text-small">
                            @if($auth->last_used_at)
                                {{ trans('settings.oauth_last_used') }}: {{ \Illuminate\Support\Carbon::parse($auth->last_used_at)->format('Y-m-d H:i') }}
                            @else
                                {{ trans('settings.oauth_never_used') }}
                            @endif
                        </div>
                        <div class="flex px-m py-xs text-right">
                            <form action="{{ url('/my-account/oauth/' . $auth->client->id) }}" method="POST">
                                {!! csrf_field() !!}
                                {!! method_field('DELETE') !!}
                                <button type="submit" class="button outline small text-neg">{{ trans('settings.oauth_revoke') }}</button>
                            </form>
                        </div>
                    </div>
                </div>
            @endforeach
        </div>
    @else
        <p class="text-muted italic py-m">{{ trans('settings.oauth_no_authorized_apps') }}</p>
    @endif
</section>
