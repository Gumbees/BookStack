<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::table('oauth_clients', function (Blueprint $table) {
            $table->string('client_secret', 128)->nullable()->after('client_id');
            $table->boolean('confidential')->default(false)->after('instance_approved');
        });

        Schema::table('oauth_access_tokens', function (Blueprint $table) {
            $table->text('scopes')->nullable()->after('client_id');
        });

        Schema::table('oauth_auth_codes', function (Blueprint $table) {
            $table->text('scopes')->nullable()->after('code_challenge_method');
        });
    }

    public function down(): void
    {
        Schema::table('oauth_clients', function (Blueprint $table) {
            $table->dropColumn(['client_secret', 'confidential']);
        });

        Schema::table('oauth_access_tokens', function (Blueprint $table) {
            $table->dropColumn('scopes');
        });

        Schema::table('oauth_auth_codes', function (Blueprint $table) {
            $table->dropColumn('scopes');
        });
    }
};
