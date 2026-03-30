<div class="backlinks-panel mb-xl"
     x-data="backlinksPanel({ entityType: '{{ $entity->getMorphClass() }}', entityId: {{ $entity->id }} })">

    <button type="button"
            class="backlinks-toggle"
            @click="open = !open"
            :aria-expanded="open.toString()">
        <h5>Backlinks<span x-show="totalCount > 0" x-text="' (' + totalCount + ')'" class="text-muted"></span></h5>
        <span class="backlinks-caret" :class="{ 'open': open }">&#9656;</span>
    </button>

    <div class="backlinks-body" x-show="open" x-cloak>
        <div x-show="loading" class="text-muted text-small py-s">Loading...</div>

        <template x-if="!loading && totalCount === 0">
            <p class="text-muted text-small">No backlinks found</p>
        </template>

        <template x-if="!loading && linked.length > 0">
            <div class="backlinks-section">
                <p class="backlinks-section-label">Linked <span x-text="'(' + linked.length + ')'"></span></p>
                <template x-for="item in linked" :key="item.type + ':' + item.id">
                    <a :href="item.url" class="backlink-item">
                        <span class="backlink-type" x-text="typeLabel(item.type)"></span>
                        <span class="backlink-name" x-text="item.name"></span>
                        <template x-if="item.breadcrumb">
                            <span class="backlink-breadcrumb" x-text="item.breadcrumb"></span>
                        </template>
                        <template x-if="item.excerpt">
                            <span class="backlink-excerpt" x-html="item.excerpt"></span>
                        </template>
                    </a>
                </template>
            </div>
        </template>

        <template x-if="!loading && unlinked.length > 0">
            <div class="backlinks-section">
                <p class="backlinks-section-label">Unlinked <span x-text="'(' + unlinked.length + ')'"></span></p>
                <template x-for="item in unlinked" :key="item.type + ':' + item.id">
                    <a :href="item.url" class="backlink-item backlink-item-unlinked">
                        <span class="backlink-type" x-text="typeLabel(item.type)"></span>
                        <span class="backlink-name" x-text="item.name"></span>
                        <template x-if="item.breadcrumb">
                            <span class="backlink-breadcrumb" x-text="item.breadcrumb"></span>
                        </template>
                        <template x-if="item.excerpt">
                            <span class="backlink-excerpt" x-html="item.excerpt"></span>
                        </template>
                    </a>
                </template>
            </div>
        </template>
    </div>

</div>
