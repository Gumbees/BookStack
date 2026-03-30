<?php

namespace BookStack\Console\Commands;

use BookStack\Activity\Notifications\InAppNotificationService;
use Illuminate\Console\Command;

class CleanupNotificationsCommand extends Command
{
    /**
     * The name and signature of the console command.
     *
     * @var string
     */
    protected $signature = 'bookstack:cleanup-notifications';

    /**
     * The console command description.
     *
     * @var string
     */
    protected $description = 'Delete in-app notifications older than the configured retention period';

    /**
     * Execute the console command.
     */
    public function handle(InAppNotificationService $service): int
    {
        $days = setting('notifications.cleanup_days', 90);
        $days = max(1, intval($days));

        $deleted = $service->cleanup($days);

        $this->comment("Cleaned up {$deleted} notification(s) older than {$days} day(s).");

        return 0;
    }
}
