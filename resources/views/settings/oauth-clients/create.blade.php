@extends('layouts.simple')

@section('body')

    <div class="container small">

        @include('settings.parts.navbar', ['selected' => 'oauth-clients'])

        <div class="card content-wrap auto-height">
            <h1 class="list-heading">{{ trans('settings.oauth_client_create') }}</h1>

            <form action="{{ url('/settings/oauth-clients') }}" method="POST">
                {!! csrf_field() !!}

                <div class="setting-list">
                    <div>
                        <label for="name" class="setting-list-label">{{ trans('common.name') }}</label>
                        <input type="text" id="name" name="name" value="{{ old('name', '') }}" required maxlength="150">
                        @if($errors->has('name'))
                            <div class="text-neg text-small mt-xs">{{ $errors->first('name') }}</div>
                        @endif
                    </div>

                    <div>
                        <label for="redirect_uris" class="setting-list-label">{{ trans('settings.oauth_redirect_uris') }}</label>
                        <p class="small text-muted">{{ trans('settings.oauth_client_create_redirect_desc') }}</p>
                        <textarea id="redirect_uris" name="redirect_uris" rows="3">{{ old('redirect_uris', '') }}</textarea>
                        @if($errors->has('redirect_uris'))
                            <div class="text-neg text-small mt-xs">{{ $errors->first('redirect_uris') }}</div>
                        @endif
                    </div>

                    <div>
                        <label class="toggle-switch-list">
                            <div>
                                <input type="hidden" name="confidential" value="false">
                                <input type="checkbox" name="confidential" value="true" {{ old('confidential') === 'true' ? 'checked' : '' }}>
                                <span class="custom-checkbox"></span>
                            </div>
                            <div class="flex">
                                <strong>{{ trans('settings.oauth_confidential') }}</strong>
                                <br>
                                <small class="text-muted">{{ trans('settings.oauth_confidential_desc') }}</small>
                            </div>
                        </label>
                    </div>

                    <div>
                        <label class="toggle-switch-list">
                            <div>
                                <input type="hidden" name="instance_approved" value="false">
                                <input type="checkbox" name="instance_approved" value="true" {{ old('instance_approved') === 'true' ? 'checked' : '' }}>
                                <span class="custom-checkbox"></span>
                            </div>
                            <div class="flex">
                                <strong>{{ trans('settings.oauth_instance_approved') }}</strong>
                                <br>
                                <small class="text-muted">{{ trans('settings.oauth_instance_approved_desc') }}</small>
                            </div>
                        </label>
                    </div>

                    <div>
                        <p class="text-warn italic">
                            {{ trans('settings.oauth_client_create_secret_warning') }}
                        </p>
                    </div>
                </div>

                <div class="form-group text-right">
                    <a href="{{ url('/settings/oauth-clients') }}" class="button outline">{{ trans('common.cancel') }}</a>
                    <button type="submit" class="button">{{ trans('common.create') }}</button>
                </div>
            </form>
        </div>
    </div>

@stop
