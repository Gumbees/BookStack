@if(!user()->isGuest())
@php
    $canEditNotes = userCan(\BookStack\Permissions\Permission::PageUpdate, $page);
    $isAdmin = user()->hasSystemRole('admin');
@endphp
<div id="scratch-notes"
     class="scratch-notes-panel mb-xl"
     x-data="scratchNotes({
         pageId: {{ $page->id }},
         canEdit: {{ $canEditNotes ? 'true' : 'false' }},
         currentUserId: {{ user()->id }},
         isAdmin: {{ $isAdmin ? 'true' : 'false' }}
     })">

    <button type="button"
            class="scratch-notes-toggle"
            @click="open = !open"
            :aria-expanded="open.toString()">
        <h5>Notes</h5>
        <span class="scratch-notes-caret" :class="{ 'open': open }">&#9656;</span>
    </button>

    <div class="scratch-notes-body" x-show="open" x-cloak>

        <div x-show="loading" class="scratch-notes-loading">Loading...</div>

        <template x-if="!loading">
            <div>
                <template x-if="notes.length === 0">
                    <p class="scratch-notes-empty">No notes yet.</p>
                </template>

                <template x-for="note in notes" :key="note.id">
                    <div class="scratch-note-item">
                        <div class="scratch-note-meta">
                            <span class="scratch-note-author" x-text="note.user_name"></span>
                            <span class="scratch-note-time" :title="note.created_at" x-text="formatTime(note.created_at)"></span>
                        </div>

                        <template x-if="editingId !== note.id">
                            <div>
                                <p class="scratch-note-content" x-text="note.content"></p>
                                <div class="scratch-note-actions" x-show="canModify(note)">
                                    <button type="button" class="scratch-note-btn" @click="startEdit(note)">Edit</button>
                                    <button type="button" class="scratch-note-btn scratch-note-btn-danger" @click="deleteNote(note)">Delete</button>
                                </div>
                            </div>
                        </template>

                        <template x-if="editingId === note.id">
                            <div>
                                <textarea class="scratch-note-input"
                                          x-model="editContent"
                                          rows="3"
                                          maxlength="2000"></textarea>
                                <div class="scratch-note-actions">
                                    <button type="button" class="scratch-note-btn" @click="saveEdit(note)">Save</button>
                                    <button type="button" class="scratch-note-btn" @click="cancelEdit()">Cancel</button>
                                </div>
                            </div>
                        </template>
                    </div>
                </template>

                @if($canEditNotes)
                    <div class="scratch-note-add">
                        <textarea class="scratch-note-input"
                                  x-model="newNote"
                                  rows="3"
                                  maxlength="2000"
                                  placeholder="Add a note..."></textarea>
                        <button type="button" class="scratch-note-btn" @click="addNote()" :disabled="!newNote.trim()">Add Note</button>
                    </div>
                @endif
            </div>
        </template>

    </div>

</div>
@endif
