<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::table('oauth_clients', function (Blueprint $table) {
            $table->boolean('instance_approved')->default(false)->after('redirect_uris');
            $table->integer('created_by')->unsigned()->nullable()->after('instance_approved');
            $table->foreign('created_by')->references('id')->on('users')->onDelete('set null');
        });

        Schema::table('oauth_access_tokens', function (Blueprint $table) {
            $table->timestamp('last_used_at')->nullable()->after('expires_at');
        });
    }

    public function down(): void
    {
        Schema::table('oauth_clients', function (Blueprint $table) {
            $table->dropForeign(['created_by']);
            $table->dropColumn(['instance_approved', 'created_by']);
        });

        Schema::table('oauth_access_tokens', function (Blueprint $table) {
            $table->dropColumn('last_used_at');
        });
    }
};
