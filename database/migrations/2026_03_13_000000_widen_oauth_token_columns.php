<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::table('oauth_access_tokens', function (Blueprint $table) {
            $table->string('token', 128)->change();
        });

        Schema::table('oauth_refresh_tokens', function (Blueprint $table) {
            $table->string('token', 128)->change();
        });

        Schema::table('oauth_auth_codes', function (Blueprint $table) {
            $table->string('code', 128)->change();
        });
    }

    public function down(): void
    {
        Schema::table('oauth_access_tokens', function (Blueprint $table) {
            $table->string('token', 64)->change();
        });

        Schema::table('oauth_refresh_tokens', function (Blueprint $table) {
            $table->string('token', 64)->change();
        });

        Schema::table('oauth_auth_codes', function (Blueprint $table) {
            $table->string('code', 80)->change();
        });
    }
};
