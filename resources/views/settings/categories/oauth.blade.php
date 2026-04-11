@extends('settings.layout')

@section('card')
    <h1 id="oauth" class="list-heading">{{ trans('settings.oauth_settings') }}</h1>
    <form action="{{ url('/settings/oauth') }}" method="POST">
        {!! csrf_field() !!}
        <input type="hidden" name="section" value="oauth">

        <div class="setting-list">

            <div class="grid half gap-xl">
                <div>
                    <label class="setting-list-label">{{ trans('settings.oauth_settings_enabled') }}</label>
                    <p class="small">{{ trans('settings.oauth_settings_enabled_desc') }}</p>
                </div>
                <div>
                    @include('form.toggle-switch', [
                        'name' => 'setting-oauth.enabled',
                        'value' => setting('oauth.enabled') !== null ? (setting('oauth.enabled') === 'true' || setting('oauth.enabled') === true) : (bool) config('oauth-provider.enabled', false),
                        'label' => trans('settings.oauth_settings_enabled'),
                    ])
                </div>
            </div>

            <div class="grid half gap-xl">
                <div>
                    <label class="setting-list-label">{{ trans('settings.oauth_settings_dynamic_reg') }}</label>
                    <p class="small">{{ trans('settings.oauth_settings_dynamic_reg_desc') }}</p>
                </div>
                <div>
                    @include('form.toggle-switch', [
                        'name' => 'setting-oauth.dynamic_registration',
                        'value' => setting('oauth.dynamic_registration') !== null ? (setting('oauth.dynamic_registration') === 'true' || setting('oauth.dynamic_registration') === true) : (bool) config('oauth-provider.dynamic_registration', false),
                        'label' => trans('settings.oauth_settings_dynamic_reg'),
                    ])
                </div>
            </div>

            <h2 class="list-heading mt-xl">{{ trans('settings.oauth_settings_token_lifetimes') }}</h2>

            <div class="grid half gap-xl">
                <div>
                    <label for="setting-oauth.access_token_ttl_days" class="setting-list-label">{{ trans('settings.oauth_settings_access_token_ttl') }}</label>
                    <p class="small">{{ trans('settings.oauth_settings_access_token_ttl_desc') }}</p>
                </div>
                <div>
                    <input
                        type="number"
                        id="setting-oauth.access_token_ttl_days"
                        name="setting-oauth.access_token_ttl_days"
                        value="{{ setting('oauth.access_token_ttl_days', (int) round(config('oauth-provider.access_token_ttl', 2592000) / 86400)) }}"
                        min="1"
                        max="3650"
                        class="setting-list-input">
                </div>
            </div>

            <div class="grid half gap-xl">
                <div>
                    <label for="setting-oauth.refresh_token_ttl_days" class="setting-list-label">{{ trans('settings.oauth_settings_refresh_token_ttl') }}</label>
                    <p class="small">{{ trans('settings.oauth_settings_refresh_token_ttl_desc') }}</p>
                </div>
                <div>
                    <input
                        type="number"
                        id="setting-oauth.refresh_token_ttl_days"
                        name="setting-oauth.refresh_token_ttl_days"
                        value="{{ setting('oauth.refresh_token_ttl_days', (int) round(config('oauth-provider.refresh_token_ttl', 7776000) / 86400)) }}"
                        min="1"
                        max="3650"
                        class="setting-list-input">
                </div>
            </div>

            <div class="grid half gap-xl">
                <div>
                    <label for="setting-oauth.default_scopes" class="setting-list-label">{{ trans('settings.oauth_settings_default_scopes') }}</label>
                    <p class="small">{{ trans('settings.oauth_settings_default_scopes_desc') }}</p>
                </div>
                <div>
                    <input
                        type="text"
                        id="setting-oauth.default_scopes"
                        name="setting-oauth.default_scopes"
                        value="{{ setting('oauth.default_scopes', 'read write') }}"
                        class="setting-list-input">
                </div>
            </div>

            <h2 class="list-heading mt-xl">{{ trans('settings.oauth_settings_sso') }}</h2>

            <div class="grid half gap-xl">
                <div>
                    <label class="setting-list-label">{{ trans('settings.oauth_settings_honor_sso') }}</label>
                    <p class="small">{{ trans('settings.oauth_settings_honor_sso_desc') }}</p>
                </div>
                <div>
                    @include('form.toggle-switch', [
                        'name' => 'setting-oauth.honor_sso_lifetime',
                        'value' => setting('oauth.honor_sso_lifetime') !== null ? (setting('oauth.honor_sso_lifetime') === 'true' || setting('oauth.honor_sso_lifetime') === true) : true,
                        'label' => trans('settings.oauth_settings_honor_sso'),
                    ])
                </div>
            </div>

        </div>

        <div class="form-group text-right">
            <button type="submit" class="button">{{ trans('settings.settings_save') }}</button>
        </div>
    </form>
@endsection
