<div class="item-list-row py-s">
    <div class="flex-container-row items-center">
        <div class="flex-2 px-m">
            <a href="{{ $client->getUrl() }}">{{ $client->name }}</a>
            <br>
            <span class="small text-muted italic">{{ $client->client_id }}</span>
        </div>
        <div class="flex px-m text-muted text-small">
            @if($client->instance_approved)
                <span class="text-pos">@icon('check-circle') {{ trans('settings.oauth_instance_approved') }}</span>
            @endif
        </div>
        <div class="flex px-m text-right text-muted text-small">
            {{ trans_choice('settings.oauth_x_active_tokens', $client->access_tokens_count, ['count' => $client->access_tokens_count]) }}
        </div>
    </div>
    @if($client->createdByUser)
        <div class="px-m text-muted italic text-limit-lines-1">
            <small>{{ trans('settings.oauth_created_by', ['user' => $client->createdByUser->name]) }} &mdash; {{ $client->created_at->format('Y-m-d') }}</small>
        </div>
    @else
        <div class="px-m text-muted italic text-limit-lines-1">
            <small>{{ trans('settings.oauth_registered_dynamically') }} &mdash; {{ $client->created_at->format('Y-m-d') }}</small>
        </div>
    @endif
</div>
