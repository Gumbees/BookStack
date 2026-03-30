@extends('users.account.layout')

@section('main')
    <section class="card content-wrap auto-height">
        <form action="{{ url('/my-account/notifications') }}" method="post">
            {{ method_field('put') }}
            {{ csrf_field() }}

            <h1 class="list-heading">{{ trans('preferences.notifications') }}</h1>
            <p class="text-small text-muted">{{ trans('preferences.notifications_desc') }}</p>

            <div class="flex-container-row wrap justify-space-between pb-m">
                <div class="toggle-switch-list min-width-l">
                    <div>
                        @include('form.toggle-switch', [
                            'name' => 'preferences[own-page-changes]',
                            'value' => $preferences->notifyOnOwnPageChanges(),
                            'label' => trans('preferences.notifications_opt_own_page_changes'),
                        ])
                    </div>
                    @if (!setting('app-disable-comments'))
                        <div>
                            @include('form.toggle-switch', [
                                'name' => 'preferences[own-page-comments]',
                                'value' => $preferences->notifyOnOwnPageComments(),
                                'label' => trans('preferences.notifications_opt_own_page_comments'),
                            ])
                        </div>
                        <div>
                            @include('form.toggle-switch', [
                                'name' => 'preferences[comment-replies]',
                                'value' => $preferences->notifyOnCommentReplies(),
                                'label' => trans('preferences.notifications_opt_comment_replies'),
                            ])
                        </div>
                        <div>
                            @include('form.toggle-switch', [
                                'name' => 'preferences[comment-mentions]',
                                'value' => $preferences->notifyOnCommentMentions(),
                                'label' => trans('preferences.notifications_opt_comment_mentions'),
                            ])
                        </div>
                    @endif
                </div>

                <div class="mt-auto">
                    <button class="button">{{ trans('preferences.notifications_save') }}</button>
                </div>
            </div>

        </form>
    </section>

    <section class="card content-wrap auto-height">
        <h2 class="list-heading">{{ trans('preferences.notifications_watched') }}</h2>
        <p class="text-small text-muted">{{ trans('preferences.notifications_watched_desc') }}</p>

        @php
            $watchLabels = json_encode([
                'comments' => trans('entities.watch_title_comments'),
                'updates'  => trans('entities.watch_title_updates'),
                'new'      => trans('entities.watch_title_new'),
                'ignore'   => trans('entities.watch_title_ignore'),
            ]);
        @endphp
        @if($watches->isEmpty())
            <p class="text-muted italic">{{ trans('common.no_items') }}</p>
        @else
            <div class="item-list">
                @foreach($watches as $watch)
                    @php $watchable = $watch->watchable; @endphp
                    @if($watchable)
                    <div class="flex-container-row justify-space-between item-list-row items-center wrap px-m py-s"
                         x-data="followButton({
                             entityType: '{{ $watchable->getMorphClass() }}',
                             entityId: {{ $watchable->id }},
                             currentLevel: {{ $watch->level }},
                             isPage: {{ $watchable instanceof \BookStack\Entities\Models\Page ? 'true' : 'false' }},
                             labels: {!! $watchLabels !!}
                         })">
                        <div class="py-xs px-s min-width-m">
                            @include('entities.icon-link', ['entity' => $watchable])
                        </div>
                        <div class="py-xs min-width-m text-m-right px-m flex-container-row items-center gap-s">
                            <span>
                                @icon('watch' . ($watch->ignoring() ? '-ignore' : ''))
                                {{ trans('entities.watch_title_' . $watch->getLevelName()) }}
                            </span>
                            <button type="button"
                                    class="button outline small"
                                    x-on:click="setLevel(-1)"
                                    :disabled="saving"
                                    x-show="level >= 0">
                                {{ trans('preferences.notifications_following_unfollow') }}
                            </button>
                        </div>
                    </div>
                    @endif
                @endforeach
            </div>
        @endif

        <div class="my-m">{{ $watches->links() }}</div>
    </section>
@stop
