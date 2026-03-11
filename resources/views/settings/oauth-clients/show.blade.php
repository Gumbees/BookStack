@extends('layouts.simple')

@section('body')

    <div class="container small">

        @include('settings.parts.navbar', ['selected' => 'oauth-clients'])

        <div class="card content-wrap auto-height">
            <div class="flex-container-row items-center justify-space-between wrap">
                <h1 class="list-heading">{{ $client->name }}</h1>
                <div>
                    <a href="{{ $client->getUrl('delete') }}" class="button outline small">{{ trans('common.delete') }}</a>
                </div>
            </div>

            <form action="{{ $client->getUrl() }}" method="POST">
                {!! csrf_field() !!}
                {!! method_field('PUT') !!}

                <div class="grid half mt-m gap-xl wrap stretch-inputs">
                    <div>
                        <label for="name">{{ trans('common.name') }}</label>
                        <input type="text" id="name" name="name" value="{{ $client->name }}" required maxlength="150">
                    </div>
                    <div>
                        <label for="client_id">{{ trans('settings.oauth_client_id') }}</label>
                        <input type="text" id="client_id" value="{{ $client->client_id }}" disabled readonly>
                    </div>
                </div>

                <div class="mt-m">
                    <label class="toggle-switch-list">
                        <div>
                            <input type="hidden" name="instance_approved" value="false">
                            <input type="checkbox" name="instance_approved" value="true" @if($client->instance_approved) checked @endif>
                            <span class="custom-checkbox"></span>
                        </div>
                        <div class="flex">
                            <strong>{{ trans('settings.oauth_instance_approved') }}</strong>
                            <br>
                            <small class="text-muted">{{ trans('settings.oauth_instance_approved_desc') }}</small>
                        </div>
                    </label>
                </div>

                @if($client->redirect_uris)
                    <div class="mt-m">
                        <label>{{ trans('settings.oauth_redirect_uris') }}</label>
                        <div class="text-muted text-small">
                            @foreach(json_decode($client->redirect_uris, true) ?? [] as $uri)
                                <code>{{ $uri }}</code><br>
                            @endforeach
                        </div>
                    </div>
                @endif

                <div class="form-group text-right mt-m">
                    <a href="{{ url('/settings/oauth-clients') }}" class="button outline">{{ trans('common.cancel') }}</a>
                    <button type="submit" class="button">{{ trans('common.save') }}</button>
                </div>
            </form>
        </div>

        @if(count($authorizations) > 0)
            <div class="card content-wrap auto-height">
                <h2 class="list-heading">{{ trans('settings.oauth_active_authorizations') }}</h2>
                <p class="text-muted text-small">{{ trans('settings.oauth_active_authorizations_desc') }}</p>

                <div class="item-list my-m">
                    @foreach($authorizations as $auth)
                        <div class="item-list-row flex-container-row items-center wrap py-xs gap-x-m">
                            <div class="flex px-m py-xs min-width-m">
                                <strong>{{ $auth->user->name }}</strong>
                                <br>
                                <span class="small text-muted">{{ $auth->user->email }}</span>
                            </div>
                            <div class="flex flex-container-row items-center min-width-m">
                                <div class="flex px-m py-xs text-muted text-small">
                                    {{ trans_choice('settings.oauth_x_active_tokens', $auth->token_count, ['count' => $auth->token_count]) }}
                                    @if($auth->last_used_at)
                                        <br>{{ trans('settings.oauth_last_used') }}: {{ \Illuminate\Support\Carbon::parse($auth->last_used_at)->format('Y-m-d H:i') }}
                                    @endif
                                </div>
                                <div class="flex px-m py-xs text-right">
                                    <form action="{{ $client->getUrl('authorization') }}" method="POST" class="inline">
                                        {!! csrf_field() !!}
                                        {!! method_field('DELETE') !!}
                                        <input type="hidden" name="user_id" value="{{ $auth->user->id }}">
                                        <button type="submit" class="button outline small text-neg">{{ trans('settings.oauth_revoke') }}</button>
                                    </form>
                                </div>
                            </div>
                        </div>
                    @endforeach
                </div>
            </div>
        @endif

    </div>

@stop
