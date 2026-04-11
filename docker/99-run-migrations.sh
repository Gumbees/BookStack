#!/usr/bin/env bash
# Custom init script for the linuxserver BookStack image.
# s6-overlay runs scripts in /custom-cont-init.d/ on container start.
# The linuxserver base image already runs migrations, but this ensures
# our fork's custom migrations are included.
cd /app/www || exit 1
php artisan migrate --force 2>&1
