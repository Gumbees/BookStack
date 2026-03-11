@extends('layouts.simple')

@section('content')

    <div class="container very-small">

        <div class="my-l">&nbsp;</div>

        <div class="card content-wrap auto-height">
            <h1 class="list-heading">Authorize Application</h1>

            <p class="mb-m">
                <strong>{{ e($clientName) }}</strong> is requesting access to your BookStack account
                as <strong>{{ e($userName) }}</strong>.
            </p>

            <p class="mb-m text-muted text-small">
                This will allow the application to read and write content using your permissions.
            </p>

            <form method="POST" action="{{ url('/oauth/authorize') }}">
                {{ csrf_field() }}

                <div class="flex-container-row gap-m justify-flex-end mt-m">
                    <button type="submit" name="action" value="deny" class="button outline">
                        Deny
                    </button>
                    <button type="submit" name="action" value="approve" class="button">
                        Authorize
                    </button>
                </div>
            </form>
        </div>
    </div>

@stop
