@extends('layouts.simple')

@section('content')

    <div class="container very-small">

        <div class="my-l">&nbsp;</div>

        <div class="card content-wrap auto-height">
            <h1 class="list-heading">{{ trans('settings.oauth_authorize_title') }}</h1>

            <p class="mb-m">
                <strong>{{ e($clientName) }}</strong> {{ trans('settings.oauth_authorize_request') }}
                <strong>{{ e($userName) }}</strong>.
            </p>

            @if(!empty($scopes))
                <div class="mb-m">
                    <p class="text-muted text-small mb-xs">{{ trans('settings.oauth_authorize_scopes') }}</p>
                    <ul class="text-small">
                        @foreach($scopes as $scope)
                            <li><strong>{{ $scope }}</strong> &mdash; {{ trans('settings.oauth_scope_' . $scope) }}</li>
                        @endforeach
                    </ul>
                </div>
            @endif

            <form method="POST" action="{{ url('/oauth/authorize') }}">
                {{ csrf_field() }}

                <div class="flex-container-row gap-m justify-flex-end mt-m">
                    <button type="submit" name="action" value="deny" class="button outline">
                        {{ trans('common.cancel') }}
                    </button>
                    <button type="submit" name="action" value="approve" class="button">
                        {{ trans('settings.oauth_authorize_approve') }}
                    </button>
                </div>
            </form>
        </div>
    </div>

@stop
