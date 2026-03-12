# Extends the official linuxserver/bookstack image with our fork's changes.
# No new composer/npm deps — we only overlay PHP code, views, routes, and migrations.
# The linuxserver init scripts handle migrations automatically on startup.
FROM lscr.io/linuxserver/bookstack:latest

# Overlay our fork's code on top of the upstream app
# Only copy directories we've modified — keeps the layer small
COPY app/       /app/www/app/
COPY database/  /app/www/database/
COPY lang/      /app/www/lang/
COPY resources/ /app/www/resources/
COPY routes/    /app/www/routes/
