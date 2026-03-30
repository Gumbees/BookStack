import {MarkdownEditor} from './index.mjs';
import {debounce} from '../services/util';
import {showLoading} from '../services/dom';

interface WikiLinkEntity {
    id: number;
    name: string;
    type: string;
    link: string;
}

/**
 * Build a permalink for an entity. Pages get /link/{id}. Others use their URL.
 */
function buildEntityLink(entity: WikiLinkEntity): string {
    if (entity.type === 'page') {
        return window.baseUrl(`/link/${entity.id}`);
    }
    return entity.link;
}

/**
 * Extract entity data from an anchor element from the entity-selector-list template.
 */
function entityFromListItem(item: HTMLAnchorElement): WikiLinkEntity {
    const type = item.getAttribute('data-entity-type') || '';
    const id = Number(item.getAttribute('data-entity-id') || 0);
    const name = item.querySelector('.entity-list-item-name')?.textContent?.trim() || '';
    const link = item.getAttribute('href') || '';
    return {id, type, name, link};
}

/**
 * Given the full text and cursor offset, return the wiki query text after `[[`
 * or null if the cursor is not in a `[[...` context.
 */
function extractWikiQuery(text: string, cursorOffset: number): string | null {
    const textToCursor = text.slice(0, cursorOffset);
    const markerIndex = textToCursor.lastIndexOf('[[');
    if (markerIndex === -1) return null;

    // Make sure there's no closing ]] or newline between [[ and cursor
    const between = textToCursor.slice(markerIndex + 2);
    if (between.includes(']]') || between.includes('\n')) return null;

    return between;
}

/**
 * Get the cursor position from the DOM selection within the CodeMirror editor.
 * Returns {x, y} of the bottom of the current cursor caret.
 */
function getCursorScreenPosition(): { x: number; y: number } | null {
    const selection = window.getSelection();
    if (!selection || selection.rangeCount === 0) return null;

    const range = selection.getRangeAt(0);
    const rect = range.getBoundingClientRect();
    return {x: rect.left, y: rect.bottom};
}

class MarkdownWikiLinkHandler {
    protected editor: MarkdownEditor;
    protected dropdown: HTMLElement | null = null;
    protected listEl: HTMLElement | null = null;
    protected active = false;
    protected selectedIndex = 0;
    protected results: WikiLinkEntity[] = [];
    protected abortController: AbortController | null = null;
    protected container: HTMLElement;

    protected searchDebounced: (query: string) => void;

    constructor(editor: MarkdownEditor) {
        this.editor = editor;
        this.container = editor.config.container as HTMLElement;
        this.searchDebounced = debounce(this.performSearch.bind(this), 200, false);
    }

    /**
     * Called on every text change. Checks if `[[` pattern is active at cursor.
     */
    handleChange(): void {
        const selection = this.editor.input.getSelection();
        const text = this.editor.input.getText();
        const query = extractWikiQuery(text, selection.from);

        if (query === null) {
            this.hide();
            return;
        }

        if (!this.active) {
            this.show();
        }

        this.searchDebounced(query);
    }

    show(): void {
        if (this.active) return;
        this.active = true;
        this.abortController = new AbortController();

        this.listEl = document.createElement('div');
        this.listEl.className = 'dropdown-search-list wiki-link-list';
        this.dropdown = document.createElement('div');
        this.dropdown.className = 'dropdown-search-dropdown compact card wiki-link-dropdown';
        this.dropdown.style.display = 'none';
        this.dropdown.style.position = 'absolute';
        this.dropdown.appendChild(this.listEl);

        showLoading(this.listEl);

        // Append to the markdown editor container
        this.container.appendChild(this.dropdown);

        this.dropdown.addEventListener('click', (event: MouseEvent) => {
            const item = (event.target as HTMLElement).closest('a[data-entity-type]') as HTMLAnchorElement | null;
            if (item) {
                event.preventDefault();
                this.selectItem(entityFromListItem(item));
            }
        }, {signal: this.abortController.signal});

        // Close on click outside
        document.addEventListener('click', (event: MouseEvent) => {
            if (!this.dropdown?.contains(event.target as Node)) {
                this.hide();
            }
        }, {signal: this.abortController.signal, capture: true});

        this.positionDropdown();
    }

    hide(): void {
        if (!this.active) return;
        this.active = false;
        this.results = [];
        this.selectedIndex = 0;
        this.abortController?.abort();
        this.abortController = null;
        this.dropdown?.remove();
        this.dropdown = null;
        this.listEl = null;
    }

    protected positionDropdown(): void {
        if (!this.dropdown) return;

        const pos = getCursorScreenPosition();
        if (!pos) return;

        const containerRect = this.container.getBoundingClientRect();
        const top = pos.y - containerRect.top + 4;
        const left = pos.x - containerRect.left;

        this.dropdown.style.top = `${top}px`;
        this.dropdown.style.left = `${left}px`;
        this.dropdown.style.right = 'auto';
        this.dropdown.style.display = 'block';
    }

    protected async performSearch(query: string): Promise<void> {
        if (!this.active || !this.listEl) return;

        showLoading(this.listEl);

        const url = `/search/entity-selector?types=page,book,chapter&permission=view&term=${encodeURIComponent(query)}`;

        try {
            const resp = await window.$http.get(url);
            if (!this.active || !this.listEl) return;

            this.listEl.innerHTML = '';
            const parser = new DOMParser();
            const doc = parser.parseFromString(resp.data as string, 'text/html');
            const items = [...doc.querySelectorAll('a[data-entity-type]')].slice(0, 8) as HTMLAnchorElement[];

            this.results = items.map(entityFromListItem);

            if (this.results.length === 0) {
                const empty = document.createElement('p');
                empty.className = 'text-muted px-m py-s';
                empty.textContent = 'No results';
                this.listEl.appendChild(empty);
            } else {
                for (const item of items) {
                    const adopted = document.adoptNode(item);
                    (adopted as HTMLElement).setAttribute('tabindex', '0');
                    this.listEl.appendChild(adopted);
                }
                this.selectedIndex = 0;
                this.updateSelection();
            }

            this.positionDropdown();
        } catch (e) {
            if (this.active && this.listEl) {
                this.listEl.innerHTML = '';
                const err = document.createElement('p');
                err.className = 'text-muted px-m py-s';
                err.textContent = 'Search failed';
                this.listEl.appendChild(err);
            }
        }
    }

    protected updateSelection(): void {
        if (!this.listEl) return;
        const items = [...this.listEl.querySelectorAll('a[data-entity-type]')] as HTMLElement[];
        items.forEach((item, i) => {
            item.classList.toggle('wiki-link-selected', i === this.selectedIndex);
        });
    }

    moveDown(): void {
        if (!this.active || this.results.length === 0) return;
        this.selectedIndex = (this.selectedIndex + 1) % this.results.length;
        this.updateSelection();
    }

    moveUp(): void {
        if (!this.active || this.results.length === 0) return;
        this.selectedIndex = (this.selectedIndex - 1 + this.results.length) % this.results.length;
        this.updateSelection();
    }

    confirmSelection(): boolean {
        if (!this.active || this.results.length === 0) return false;
        const entity = this.results[this.selectedIndex];
        if (!entity) return false;
        this.selectItem(entity);
        return true;
    }

    protected selectItem(entity: WikiLinkEntity): void {
        const selection = this.editor.input.getSelection();
        const text = this.editor.input.getText();
        const textToCursor = text.slice(0, selection.from);
        const markerIndex = textToCursor.lastIndexOf('[[');
        if (markerIndex === -1) {
            this.hide();
            return;
        }

        const url = buildEntityLink(entity);
        const replacement = `[${entity.name}](${url})`;

        this.hide();

        this.editor.input.spliceText(
            markerIndex,
            selection.from,
            replacement,
            {from: markerIndex + replacement.length, to: markerIndex + replacement.length},
        );
    }

    isActive(): boolean {
        return this.active;
    }
}

/**
 * Register wiki-link `[[` typeahead for the markdown editor.
 * Returns a teardown function.
 */
export function registerMarkdownWikiLinks(editor: MarkdownEditor): () => void {
    const handler = new MarkdownWikiLinkHandler(editor);
    const editorContainer = editor.config.container as HTMLElement;
    const inputEl = editor.config.inputEl;
    const abortController = new AbortController();

    const handleInput = () => handler.handleChange();

    // Input events bubble up from CodeMirror's .cm-content contenteditable and
    // from the plain textarea, so one listener on the container covers both modes.
    editorContainer.addEventListener('input', handleInput, {signal: abortController.signal});

    // Also listen on the raw textarea to catch plain-editor mode reliably
    // (the textarea may not be inside the container in all configurations).
    inputEl.addEventListener('input', handleInput, {signal: abortController.signal});

    // Keyboard navigation (capture phase so we intercept before CodeMirror)
    const keyHandler = (event: KeyboardEvent) => {
        if (!handler.isActive()) return;

        if (event.key === 'ArrowDown') {
            event.preventDefault();
            event.stopPropagation();
            handler.moveDown();
        } else if (event.key === 'ArrowUp') {
            event.preventDefault();
            event.stopPropagation();
            handler.moveUp();
        } else if (event.key === 'Enter') {
            if (handler.confirmSelection()) {
                event.preventDefault();
                event.stopPropagation();
            }
        } else if (event.key === 'Escape') {
            handler.hide();
            event.preventDefault();
        }
    };

    editorContainer.addEventListener('keydown', keyHandler, {signal: abortController.signal, capture: true});

    return (): void => {
        abortController.abort();
        handler.hide();
    };
}
