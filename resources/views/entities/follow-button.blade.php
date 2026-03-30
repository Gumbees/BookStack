{{--
    Follow button component.

    Expected variables:
    - $entity    : The Entity model instance (Bookshelf, Book, Chapter, or Page)
    - $watchOptions : UserEntityWatchOptions instance for the current user

    Shows a prominent Follow/Following button with a level-selection dropdown.
    Works for all entity types including shelves.
--}}
@if($watchOptions->canWatch())
@php
    $currentLevel = $watchOptions->getWatchLevelValue();
    $isFollowing  = $currentLevel !== \BookStack\Activity\WatchLevels::DEFAULT && $currentLevel !== null;
    $isIgnoring   = $currentLevel === \BookStack\Activity\WatchLevels::IGNORE;
    $isPage       = $entity instanceof \BookStack\Entities\Models\Page;
    $entityType   = $entity->getMorphClass();
    $entityId     = $entity->id;
    $followLabels = json_encode([
        'comments' => trans('entities.watch_title_comments'),
        'updates'  => trans('entities.watch_title_updates'),
        'new'      => trans('entities.watch_title_new'),
        'ignore'   => trans('entities.watch_title_ignore'),
    ]);
    $followLabelFollow    = json_encode(trans('entities.follow'));
    $followLabelFollowing = json_encode(trans('entities.following'));
    $followLabelIgnore    = json_encode(trans('entities.watch_title_ignore'));
@endphp
<div class="follow-button-wrap"
     x-data="followButton({
         entityType: '{{ $entityType }}',
         entityId: {{ $entityId }},
         currentLevel: {{ $currentLevel ?? -1 }},
         isPage: {{ $isPage ? 'true' : 'false' }},
         labels: {!! $followLabels !!}
     })"
     x-on:click.outside="closeDropdown()"
     x-on:keydown.escape.window="closeDropdown()">

    {{-- Main button --}}
    <button type="button"
            class="follow-button"
            :class="{ 'following': isFollowing, 'ignoring': isIgnoring, 'saving': saving }"
            :disabled="saving"
            x-on:click="$event.stopPropagation(); dropdownOpen = !dropdownOpen"
            :aria-expanded="dropdownOpen"
            aria-haspopup="menu">
        <span class="follow-button-icon" aria-hidden="true">
            <span x-show="!isFollowing && !isIgnoring">@icon('watch')</span>
            <span x-show="isFollowing">@icon('check-circle')</span>
            <span x-show="isIgnoring">@icon('watch-ignore')</span>
        </span>
        <span class="follow-button-label"
              x-text="isFollowing ? {!! $followLabelFollowing !!} : (isIgnoring ? {!! $followLabelIgnore !!} : {!! $followLabelFollow !!})">
        </span>
        <span class="follow-button-caret">@icon('caret-down')</span>
    </button>

    {{-- Dropdown menu --}}
    <div class="follow-dropdown"
         x-show="dropdownOpen"
         x-transition:enter="follow-dropdown-enter"
         x-transition:leave="follow-dropdown-leave"
         role="menu"
         style="display:none;">

        <div class="follow-dropdown-header">{{ trans('entities.follow_level_label') }}</div>

        <template x-for="opt in levels()" :key="opt.value">
            <button type="button"
                    class="follow-level-option"
                    role="menuitem"
                    x-on:click="setLevel(opt.value)">
                <span class="follow-level-check">
                    <span x-show="level === opt.value">@icon('check-circle')</span>
                </span>
                <span x-text="opt.label"></span>
            </button>
        </template>

        <hr class="follow-dropdown-divider">

        <button type="button"
                class="follow-level-option follow-unfollow-option"
                role="menuitem"
                x-show="isFollowing || isIgnoring"
                x-on:click="setLevel(-1)">
            <span class="follow-level-check"></span>
            <span>{{ trans('entities.unfollow') }}</span>
        </button>

        <a href="{{ url('/my-account/notifications') }}"
           class="follow-dropdown-footer-link"
           target="_blank"
           role="menuitem">{{ trans('entities.watch_change_default') }}</a>
    </div>
</div>
@endif
