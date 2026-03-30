<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Support\Facades\DB;

return new class extends Migration
{
    protected array $defaults = [
        'notifications.in_app_enabled'           => 'true',
        'notifications.email_enabled'            => 'true',
        'notifications.mention_enabled'          => 'true',
        'notifications.default_own_page_changes' => 'true',
        'notifications.default_own_page_comments'=> 'true',
        'notifications.default_comment_replies'  => 'true',
        'notifications.default_comment_mentions' => 'true',
        'notifications.cleanup_days'             => '90',
    ];

    /**
     * Run the migrations.
     */
    public function up(): void
    {
        $now = now();

        foreach ($this->defaults as $key => $value) {
            $exists = DB::table('settings')->where('setting_key', $key)->exists();
            if (!$exists) {
                DB::table('settings')->insert([
                    'setting_key' => $key,
                    'value'       => $value,
                    'created_at'  => $now,
                    'updated_at'  => $now,
                ]);
            }
        }
    }

    /**
     * Reverse the migrations.
     */
    public function down(): void
    {
        DB::table('settings')
            ->whereIn('setting_key', array_keys($this->defaults))
            ->delete();
    }
};
