@extends('layouts.simple')

@section('body')
    <div class="container small">

        @include('settings.parts.navbar', ['selected' => 'oauth-clients'])

        <div class="card content-wrap auto-height">
            <h1 class="list-heading">{{ trans('settings.oauth_client_delete') }}</h1>

            <p>{{ trans('settings.oauth_client_delete_warning', ['clientName' => $client->name]) }}</p>

            <form action="{{ $client->getUrl() }}" method="POST">
                {!! csrf_field() !!}
                {!! method_field('DELETE') !!}

                <div class="grid half v-center">
                    <div>
                        <p class="text-neg">
                            <strong>{{ trans('settings.oauth_client_delete_confirm') }}</strong>
                        </p>
                    </div>
                    <div>
                        <div class="form-group text-right">
                            <a href="{{ $client->getUrl() }}" class="button outline">{{ trans('common.cancel') }}</a>
                            <button type="submit" class="button">{{ trans('common.confirm') }}</button>
                        </div>
                    </div>
                </div>
            </form>
        </div>

    </div>
@stop
