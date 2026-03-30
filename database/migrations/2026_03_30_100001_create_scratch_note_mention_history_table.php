<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    /**
     * Run the migrations.
     */
    public function up(): void
    {
        Schema::create('scratch_note_mention_history', function (Blueprint $table) {
            $table->id();
            $table->unsignedBigInteger('scratch_note_id')->index();
            $table->unsignedInteger('user_id')->index();
            $table->timestamp('created_at')->nullable();

            $table->unique(['scratch_note_id', 'user_id']);
        });
    }

    /**
     * Reverse the migrations.
     */
    public function down(): void
    {
        Schema::dropIfExists('scratch_note_mention_history');
    }
};
