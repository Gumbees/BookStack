@extends('layouts.simple')

@section('body')

    <div class="container small">

        @include('settings.parts.navbar', ['selected' => 'oauth-clients'])

        <div class="card content-wrap auto-height">

            <div class="flex-container-row items-center justify-space-between wrap">
                <h1 class="list-heading">{{ trans('settings.oauth_clients') }}</h1>
                <div>
                    <a href="{{ url('/settings/oauth-clients/create') }}" class="button outline">{{ trans('settings.oauth_client_create') }}</a>
                </div>
            </div>

            <p class="text-muted">{{ trans('settings.oauth_clients_desc') }}</p>

            @if(count($clients) > 0)
                <div class="item-list my-m">
                    @foreach($clients as $client)
                        @include('settings.oauth-clients.parts.client-list-item', ['client' => $client])
                    @endforeach
                </div>
            @else
                <p class="text-muted italic py-m">
                    {{ trans('settings.oauth_clients_none') }}
                </p>
            @endif

        </div>
    </div>

@stop
