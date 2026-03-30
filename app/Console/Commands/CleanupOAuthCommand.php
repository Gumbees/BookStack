<?php

namespace BookStack\Console\Commands;

use BookStack\OAuth\OAuthService;
use Illuminate\Console\Command;

class CleanupOAuthCommand extends Command
{
    /**
     * The name and signature of the console command.
     *
     * @var string
     */
    protected $signature = 'bookstack:cleanup-oauth';

    /**
     * The console command description.
     *
     * @var string
     */
    protected $description = 'Clean up expired OAuth tokens and authorization codes';

    /**
     * Execute the console command.
     */
    public function handle(OAuthService $oauthService): int
    {
        $oauthService->cleanupExpired();
        $this->comment('Expired OAuth tokens and codes cleaned up');
        return 0;
    }
}
