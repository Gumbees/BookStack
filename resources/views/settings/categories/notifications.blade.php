@extends('settings.layout')

@section('card')
    <h1 id="notifications" class="list-heading">Notifications</h1>
    <form action="{{ url('/settings/notifications') }}" method="POST">
        {!! csrf_field() !!}
        <input type="hidden" name="section" value="notifications">

        <div class="setting-list">

            <div class="grid half gap-xl">
                <div>
                    <label class="setting-list-label">Enable in-app notifications</label>
                    <p class="small">Show a notification bell in the header for users to receive in-app notifications about page changes, comments, and mentions.</p>
                </div>
                <div>
                    @include('form.toggle-switch', [
                        'name' => 'setting-notifications.in_app_enabled',
                        'value' => setting('notifications.in_app_enabled', true),
                        'label' => 'In-app notifications enabled',
                    ])
                </div>
            </div>

            <div class="grid half gap-xl">
                <div>
                    <label class="setting-list-label">Enable email notifications</label>
                    <p class="small">Send email notifications to users when they are watching pages or are mentioned. Disabling this stops all notification emails.</p>
                </div>
                <div>
                    @include('form.toggle-switch', [
                        'name' => 'setting-notifications.email_enabled',
                        'value' => setting('notifications.email_enabled', true),
                        'label' => 'Email notifications enabled',
                    ])
                </div>
            </div>

            <div class="grid half gap-xl">
                <div>
                    <label class="setting-list-label">Enable @mentions</label>
                    <p class="small">Allow users to @mention other users in comments and scratch notes to trigger notifications.</p>
                </div>
                <div>
                    @include('form.toggle-switch', [
                        'name' => 'setting-notifications.mention_enabled',
                        'value' => setting('notifications.mention_enabled', true),
                        'label' => '@mentions enabled',
                    ])
                </div>
            </div>

            <h2 class="list-heading mt-xl">Default user preferences</h2>
            <p class="small">These defaults apply when a user has not personally configured their notification preferences.</p>

            <div class="grid half gap-xl">
                <div>
                    <label class="setting-list-label">Notify on own page changes</label>
                    <p class="small">Default setting for whether users are notified when their own pages are updated.</p>
                </div>
                <div>
                    @include('form.toggle-switch', [
                        'name' => 'setting-notifications.default_own_page_changes',
                        'value' => setting('notifications.default_own_page_changes', true),
                        'label' => 'Default: notify on own page changes',
                    ])
                </div>
            </div>

            <div class="grid half gap-xl">
                <div>
                    <label class="setting-list-label">Notify on own page comments</label>
                    <p class="small">Default setting for whether users are notified when comments are added to their own pages.</p>
                </div>
                <div>
                    @include('form.toggle-switch', [
                        'name' => 'setting-notifications.default_own_page_comments',
                        'value' => setting('notifications.default_own_page_comments', true),
                        'label' => 'Default: notify on own page comments',
                    ])
                </div>
            </div>

            <div class="grid half gap-xl">
                <div>
                    <label class="setting-list-label">Notify on comment replies</label>
                    <p class="small">Default setting for whether users are notified when someone replies to their comment.</p>
                </div>
                <div>
                    @include('form.toggle-switch', [
                        'name' => 'setting-notifications.default_comment_replies',
                        'value' => setting('notifications.default_comment_replies', true),
                        'label' => 'Default: notify on comment replies',
                    ])
                </div>
            </div>

            <div class="grid half gap-xl">
                <div>
                    <label class="setting-list-label">Notify on comment mentions</label>
                    <p class="small">Default setting for whether users are notified when they are @mentioned in a comment.</p>
                </div>
                <div>
                    @include('form.toggle-switch', [
                        'name' => 'setting-notifications.default_comment_mentions',
                        'value' => setting('notifications.default_comment_mentions', true),
                        'label' => 'Default: notify on comment mentions',
                    ])
                </div>
            </div>

            <h2 class="list-heading mt-xl">Notification retention</h2>

            <div class="grid half gap-xl">
                <div>
                    <label for="setting-notifications.cleanup_days" class="setting-list-label">Days to retain notifications</label>
                    <p class="small">In-app notifications older than this many days will be automatically deleted by the daily cleanup job.</p>
                </div>
                <div>
                    <input
                        type="number"
                        id="setting-notifications.cleanup_days"
                        name="setting-notifications.cleanup_days"
                        value="{{ setting('notifications.cleanup_days', 90) }}"
                        min="1"
                        max="3650"
                        class="setting-list-input">
                </div>
            </div>

        </div>

        <div class="form-group text-right">
            <button type="submit" class="button">{{ trans('settings.settings_save') }}</button>
        </div>
    </form>
@endsection
