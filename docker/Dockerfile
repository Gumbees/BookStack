# Production Dockerfile for the BookStack fork.
#
# Build from the repo root (NOT from the docker/ directory):
#   docker build -f docker/Dockerfile .
#
# Overlay approach: starts from the official linuxserver/bookstack image and
# copies only the directories this fork has modified on top of the upstream app.
# No new composer/npm dependencies -- only PHP code, views, routes, and migrations
# are changed, so we skip a full composer/npm build step.
#
# Version alignment: the base image must match the upstream BookStack release this
# fork is built on. This fork tracks v25.12.x. If you rebase onto a newer upstream
# release, update the tag below to match that release before deploying.
FROM lscr.io/linuxserver/bookstack:25.12

# Overlay our fork's code on top of the upstream app.
# Only copy directories we've modified -- keeps the layer small.
COPY app/       /app/www/app/
COPY database/  /app/www/database/
COPY lang/      /app/www/lang/
COPY resources/ /app/www/resources/
COPY routes/    /app/www/routes/

# Ensure the overlaid files are world-readable/executable.
# The linuxserver init process runs as abc (uid 99) but PHP-FPM worker threads
# need read access to these directories as well.
RUN chmod -R a+rX /app/www/app /app/www/database /app/www/lang /app/www/resources /app/www/routes