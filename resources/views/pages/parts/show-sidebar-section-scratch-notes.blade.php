@if(!user()->isGuest())
<div id="scratch-notes"
     class="scratch-notes-panel mb-xl"
     x-data="scratchNotes({ pageId: {{ $page->id }} })">

    <button type="button"
            class="scratch-notes-toggle"
            @click="open = !open"
            :aria-expanded="open.toString()">
        <h5>Scratch Notes</h5>
        <span class="scratch-notes-caret" :class="{ 'open': open }">&#9656;</span>
    </button>

    <div class="scratch-notes-body" x-show="open" x-cloak>
        <textarea class="scratch-notes-textarea"
                  x-model="content"
                  @input.debounce.1000ms="save()"
                  placeholder="Personal notes for this page..."
                  :disabled="loading"></textarea>
        <div class="scratch-notes-status" x-text="statusText"></div>
    </div>

</div>
@endif
