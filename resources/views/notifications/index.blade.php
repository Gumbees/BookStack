@extends('layouts.simple')

@section('body')
    <div class="container medium">
        <div class="my-xl">

            <div class="flex-container-row justify-space-between items-center mb-m">
                <h1 class="list-heading mb-none">{{ trans('preferences.notifications') }}</h1>
                <div class="flex-container-row gap-s items-center">
                    @if($unreadCount > 0)
                        <form action="{{ url('/ajax/notifications/mark-all-read') }}" method="post" x-data>
                            {{ csrf_field() }}
                            <button
                                type="submit"
                                class="button outline"
                                x-on:click.prevent="
                                    window.$http.post('/ajax/notifications/mark-all-read').then(() => window.location.reload());
                                ">
                                Mark all as read
                            </button>
                        </form>
                    @endif
                </div>
            </div>

            <div class="mb-m flex-container-row gap-s">
                <a href="{{ url('/notifications?filter=all') }}"
                   class="button {{ $filter === 'all' ? '' : 'outline' }}">
                    All
                </a>
                <a href="{{ url('/notifications?filter=unread') }}"
                   class="button {{ $filter === 'unread' ? '' : 'outline' }}">
                    Unread @if($unreadCount > 0) ({{ $unreadCount }}) @endif
                </a>
            </div>

            <div
                class="notification-page card content-wrap"
                x-data="notificationList({ filter: '{{ $filter }}' })">

                @php
                    $notifData = $notifications->map(function($n) {
                        return [
                            'id'         => $n->id,
                            'type'       => $n->type,
                            'data'       => $n->data,
                            'read'       => !is_null($n->read_at),
                            'created_at' => $n->created_at?->toIso8601String(),
                        ];
                    })->values()->all();
                @endphp
                <script>window.__initialNotifications = @json($notifData);</script>

                @if($notifications->isEmpty())
                    <p class="text-muted italic">{{ trans('common.no_items') }}</p>
                @else
                    <div class="item-list">
                        <template x-for="notification in notifications" :key="notification.id">
                            <div
                                class="notification-item item-list-row flex-container-row items-center justify-space-between px-m py-s"
                                :class="{ 'unread': !notification.read }">

                                <div
                                    class="flex-container-row gap-m items-center flex-fill"
                                    style="cursor: pointer;"
                                    @click="clickAndNavigate(notification)"
                                    role="button"
                                    tabindex="0"
                                    @keydown.enter="clickAndNavigate(notification)">
                                    <div>
                                        <div class="notification-item-title" x-text="notification.data.title || ''"></div>
                                        <div class="notification-item-message text-small text-muted" x-text="notification.data.message || ''"></div>
                                        <div class="notification-item-time text-small text-muted" x-text="relativeTime(notification.created_at)"></div>
                                    </div>
                                </div>

                                <div class="flex-container-row gap-s items-center">
                                    <button
                                        type="button"
                                        class="text-button text-small"
                                        x-show="!notification.read"
                                        @click.stop="markAsRead(notification)"
                                        title="Mark as read">
                                        @icon('check')
                                    </button>
                                    <button
                                        type="button"
                                        class="text-button text-small"
                                        @click.stop="deleteNotification(notification)"
                                        title="Delete">
                                        @icon('close')
                                    </button>
                                </div>
                            </div>
                        </template>
                    </div>
                @endif

            </div>

            @if($totalPages > 1)
                <div class="mt-m flex-container-row justify-center gap-s">
                    @for($p = 1; $p <= $totalPages; $p++)
                        <a href="{{ url('/notifications?filter=' . $filter . '&page=' . $p) }}"
                           class="button {{ $p === $currentPage ? '' : 'outline' }}">
                            {{ $p }}
                        </a>
                    @endfor
                </div>
            @endif

        </div>
    </div>
@stop
