<?php

namespace BookStack\Activity\Notifications\Handlers;

use BookStack\Activity\ActivityType;
use BookStack\Activity\Models\Activity;
use BookStack\Activity\Models\Comment;
use BookStack\Activity\Models\Loggable;
use BookStack\Activity\Models\MentionHistory;
use BookStack\Activity\Notifications\InAppNotificationService;
use BookStack\Activity\Tools\EntityWatchers;
use BookStack\Activity\Tools\MentionParser;
use BookStack\Activity\WatchLevels;
use BookStack\Entities\Models\Entity;
use BookStack\Entities\Models\Page;
use BookStack\Permissions\Permission;
use BookStack\Permissions\PermissionApplicator;
use BookStack\Settings\UserNotificationPreferences;
use BookStack\Users\Models\User;
use Illuminate\Support\Facades\Log;

class InAppNotificationHandler implements NotificationHandler
{
    public function handle(Activity $activity, Loggable|string $detail, User $user): void
    {
        if (!setting('notifications.in_app_enabled', true)) {
            return;
        }

        match ($activity->type) {
            ActivityType::PAGE_CREATE   => $this->handlePageCreate($activity, $detail, $user),
            ActivityType::PAGE_UPDATE   => $this->handlePageUpdate($activity, $detail, $user),
            ActivityType::COMMENT_CREATE,
            ActivityType::COMMENT_UPDATE => $this->handleComment($activity, $detail, $user),
            default => null,
        };
    }

    protected function handlePageCreate(Activity $activity, Loggable|string $detail, User $initiator): void
    {
        if (!($detail instanceof Page)) {
            return;
        }

        $watchers = new EntityWatchers($detail, WatchLevels::NEW);
        $userIds = $watchers->getWatcherUserIds();

        $this->sendToUserIds($userIds, $initiator, $detail, $detail, 'page_create', [
            'title'             => $detail->name,
            'message'           => 'A new page was created.',
            'link'              => $detail->getUrl(),
            'entity_id'         => $detail->id,
            'entity_type'       => $detail->getMorphClass(),
            'triggered_by_id'   => $initiator->id,
            'triggered_by_name' => $initiator->name,
        ]);
    }

    protected function handlePageUpdate(Activity $activity, Loggable|string $detail, User $initiator): void
    {
        if (!($detail instanceof Page)) {
            return;
        }

        // Mirror the debounce from the email handler
        /** @var ?Activity $lastUpdate */
        $lastUpdate = $detail->activity()
            ->where('type', '=', ActivityType::PAGE_UPDATE)
            ->where('id', '!=', $activity->id)
            ->latest('created_at')
            ->first();

        if ($lastUpdate && $lastUpdate->user_id === $initiator->id) {
            if ($lastUpdate->created_at->gt(now()->subMinutes(15))) {
                return;
            }
        }

        $watchers = new EntityWatchers($detail, WatchLevels::UPDATES);
        $userIds = $watchers->getWatcherUserIds();

        if ($detail->owned_by && !$watchers->isUserIgnoring($detail->owned_by) && $detail->ownedBy) {
            $prefs = new UserNotificationPreferences($detail->ownedBy);
            if ($prefs->notifyOnOwnPageChanges()) {
                $userIds[] = $detail->owned_by;
            }
        }

        $this->sendToUserIds($userIds, $initiator, $detail, $detail, 'page_update', [
            'title'             => $detail->name,
            'message'           => 'A page you are watching was updated.',
            'link'              => $detail->getUrl(),
            'entity_id'         => $detail->id,
            'entity_type'       => $detail->getMorphClass(),
            'triggered_by_id'   => $initiator->id,
            'triggered_by_name' => $initiator->name,
        ]);
    }

    protected function handleComment(Activity $activity, Loggable|string $detail, User $initiator): void
    {
        if (!($detail instanceof Comment) || !($detail->entity instanceof Page)) {
            return;
        }

        /** @var Page $page */
        $page = $detail->entity;

        // --- Comment creation/reply notifications ---
        if ($activity->type === ActivityType::COMMENT_CREATE) {
            $watchers = new EntityWatchers($page, WatchLevels::COMMENTS);
            $userIds = $watchers->getWatcherUserIds();

            if ($page->owned_by && !$watchers->isUserIgnoring($page->owned_by) && $page->ownedBy) {
                $prefs = new UserNotificationPreferences($page->ownedBy);
                if ($prefs->notifyOnOwnPageComments()) {
                    $userIds[] = $page->owned_by;
                }
            }

            $parentComment = $detail->parent()->first();
            if ($parentComment && $parentComment->created_by && !$watchers->isUserIgnoring($parentComment->created_by) && $parentComment->createdBy) {
                $parentPrefs = new UserNotificationPreferences($parentComment->createdBy);
                if ($parentPrefs->notifyOnCommentReplies()) {
                    $userIds[] = $parentComment->created_by;
                }
            }

            $this->sendToUserIds($userIds, $initiator, $detail, $page, 'comment_create', [
                'title'             => $page->name,
                'message'           => 'A new comment was posted on a page you are watching.',
                'link'              => $page->getUrl('#comment' . $detail->local_id),
                'entity_id'         => $page->id,
                'entity_type'       => $page->getMorphClass(),
                'triggered_by_id'   => $initiator->id,
                'triggered_by_name' => $initiator->name,
            ]);
        }

        // --- Mention notifications ---
        if (!setting('notifications.mention_enabled', true)) {
            return;
        }

        $parser = new MentionParser();
        $mentionedUserIds = $parser->parseUserIdsFromHtml($detail->html);

        if (empty($mentionedUserIds)) {
            return;
        }

        // On comment updates, skip users who were already notified (reuse mention_history dedup)
        if ($activity->type === ActivityType::COMMENT_UPDATE) {
            $previouslyNotified = MentionHistory::query()
                ->where('mentionable_id', $detail->id)
                ->where('mentionable_type', $detail->getMorphClass())
                ->pluck('to_user_id')
                ->toArray();
            $mentionedUserIds = array_values(array_diff($mentionedUserIds, $previouslyNotified));
        }

        if (empty($mentionedUserIds)) {
            return;
        }

        $mentionedUsers = User::query()->whereIn('id', $mentionedUserIds)->get();

        $receivingUsers = $mentionedUsers->filter(function (User $mentionedUser) use ($initiator) {
            if ($mentionedUser->id === $initiator->id) {
                return false;
            }
            $prefs = new UserNotificationPreferences($mentionedUser);
            return $prefs->notifyOnCommentMentions();
        });

        $service = new InAppNotificationService();

        foreach ($receivingUsers as $recipient) {
            if (!$recipient->can(Permission::ReceiveNotifications)) {
                continue;
            }
            $permissions = new PermissionApplicator($recipient);
            if (!$permissions->checkOwnableUserAccess($page, 'view')) {
                continue;
            }

            try {
                $service->notify($recipient, 'mention', [
                    'title'             => $page->name,
                    'message'           => $initiator->name . ' mentioned you in a comment.',
                    'link'              => $page->getUrl('#comment' . $detail->local_id),
                    'entity_id'         => $page->id,
                    'entity_type'       => $page->getMorphClass(),
                    'triggered_by_id'   => $initiator->id,
                    'triggered_by_name' => $initiator->name,
                ]);
            } catch (\Exception $e) {
                Log::error("Failed to create in-app mention notification for user [id:{$recipient->id}]: {$e->getMessage()}");
            }
        }
    }

    /**
     * Send notifications to a list of user IDs, respecting permissions.
     */
    protected function sendToUserIds(
        array $userIds,
        User $initiator,
        Loggable|string $detail,
        Entity $relatedModel,
        string $type,
        array $data
    ): void {
        $users = User::query()->whereIn('id', array_unique($userIds))->get();
        $service = new InAppNotificationService();

        foreach ($users as $recipient) {
            if ($recipient->id === $initiator->id) {
                continue;
            }

            if (!$recipient->can(Permission::ReceiveNotifications)) {
                continue;
            }

            $permissions = new PermissionApplicator($recipient);
            if (!$permissions->checkOwnableUserAccess($relatedModel, 'view')) {
                continue;
            }

            try {
                $service->notify($recipient, $type, $data);
            } catch (\Exception $e) {
                Log::error("Failed to create in-app notification for user [id:{$recipient->id}] with error: {$e->getMessage()}");
            }
        }
    }
}
