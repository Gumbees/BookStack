/**
 * Alpine.js component data factory for the follow/unfollow button.
 * Handles all entity types including shelves via the /ajax/follow/{type}/{id} endpoints.
 *
 * Level values: -1=not following (default), 0=ignore, 1=new content, 2=updates, 3=all activity
 *
 * @param {Object} params
 * @param {string} params.entityType - e.g. "bookshelf", "book", "chapter", "page"
 * @param {number} params.entityId
 * @param {number} params.currentLevel - -1 = not following
 * @param {boolean} params.isPage - pages have no "new content" level option
 * @param {Object} params.labels - translated label strings passed from Blade
 * @returns {Object}
 */
export function followButton({ entityType, entityId, currentLevel, isPage, labels }) {
    return {
        level: currentLevel,
        dropdownOpen: false,
        saving: false,

        get isFollowing() {
            return this.level > 0;
        },

        get isIgnoring() {
            return this.level === 0;
        },

        levels() {
            const opts = [
                { value: 3, label: labels.comments },
                { value: 2, label: labels.updates },
            ];
            if (!isPage) {
                opts.push({ value: 1, label: labels.new });
            }
            opts.push({ value: 0, label: labels.ignore });
            return opts;
        },

        async setLevel(newLevel) {
            if (this.saving) return;
            this.saving = true;
            try {
                if (newLevel < 0) {
                    await window.$http.delete(`/ajax/follow/${entityType}/${entityId}`);
                    this.level = -1;
                } else {
                    await window.$http.put(`/ajax/follow/${entityType}/${entityId}`, { level: newLevel });
                    this.level = newLevel;
                }
            } catch (err) {
                // Silently fail - the follow state stays as-is
            } finally {
                this.saving = false;
                this.dropdownOpen = false;
            }
        },

        closeDropdown() {
            this.dropdownOpen = false;
        },
    };
}
