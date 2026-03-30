import {
    $createTextNode,
    $getSelection,
    $isRangeSelection,
    COMMAND_PRIORITY_NORMAL,
    KEY_ARROW_DOWN_COMMAND,
    KEY_ARROW_UP_COMMAND,
    KEY_ENTER_COMMAND,
    KEY_ESCAPE_COMMAND,
    LexicalEditor,
    RangeSelection,
    TextNode,
} from 'lexical';
import {$createLinkNode} from '@lexical/link';
import {EditorUiContext} from '../ui/framework/core';
import {el} from '../utils/dom';
import {debounce} from '../../services/util';
import {showLoading} from '../../services/dom';

interface WikiLinkEntity {
    id: number;
    name: string;
    type: string;
    link: string;
}

/**
 * Build a permalink for an entity. Pages have /link/{id} permalinks.
 * Books, chapters and shelves use their regular URL.
 */
function buildEntityLink(entity: WikiLinkEntity): string {
    if (entity.type === 'page') {
        return window.baseUrl(`/link/${entity.id}`);
    }
    return entity.link;
}

/**
 * Extract entity data from an anchor element rendered by the entity-selector-list template.
 */
function entityFromListItem(item: HTMLAnchorElement): WikiLinkEntity {
    const type = item.getAttribute('data-entity-type') || '';
    const id = Number(item.getAttribute('data-entity-id') || 0);
    const name = item.querySelector('.entity-list-item-name')?.textContent?.trim() || '';
    const link = item.getAttribute('href') || '';
    return {id, type, name, link};
}

/**
 * Build the wiki-link dropdown element and append it to the given container.
 * Returns the dropdown element.
 */
function buildDropdown(container: HTMLElement): HTMLElement {
    const list = el('div', {class: 'dropdown-search-list wiki-link-list'});
    const dropdown = el('div', {class: 'dropdown-search-dropdown compact card wiki-link-dropdown'}, [list]);
    dropdown.style.display = 'none';
    container.appendChild(dropdown);
    return dropdown;
}

/**
 * Position the dropdown relative to the current cursor position in the editor DOM.
 */
function positionDropdown(dropdown: HTMLElement, editorDOM: HTMLElement, containerDOM: HTMLElement): void {
    const selection = window.getSelection();
    if (!selection || selection.rangeCount === 0) {
        return;
    }

    const range = selection.getRangeAt(0);
    const rect = range.getBoundingClientRect();
    const containerRect = containerDOM.getBoundingClientRect();

    const top = rect.bottom - containerRect.top + 4;
    const left = rect.left - containerRect.left;

    dropdown.style.top = `${top}px`;
    dropdown.style.left = `${left}px`;
    dropdown.style.right = 'auto';
    dropdown.style.display = 'block';
    dropdown.style.position = 'absolute';
}

/**
 * Extract the query text typed after `[[` up to the current cursor position.
 * Returns null if cursor is not in a `[[...` pattern.
 */
function extractWikiQuery(node: TextNode, selection: RangeSelection): string | null {
    const points = selection.getStartEndPoints();
    if (!points) return null;

    const offset = points[0].offset;
    const text = node.getTextContent();
    const textToCursor = text.slice(0, offset);

    const markerIndex = textToCursor.lastIndexOf('[[');
    if (markerIndex === -1) return null;

    // Make sure there's no closing ]] between [[ and cursor
    const between = textToCursor.slice(markerIndex + 2);
    if (between.includes(']]')) return null;

    return between;
}

/**
 * Replace the `[[query` text in the current text node with a link node.
 */
function insertWikiLink(editor: LexicalEditor, entity: WikiLinkEntity): void {
    editor.update(() => {
        const selection = $getSelection();
        if (!$isRangeSelection(selection) || !selection.isCollapsed()) {
            return;
        }

        const nodes = selection.getNodes();
        if (nodes.length === 0) return;

        const textNode = nodes[0];
        if (!(textNode instanceof TextNode)) return;

        const points = selection.getStartEndPoints();
        if (!points) return;

        const offset = points[0].offset;
        const text = textNode.getTextContent();
        const textToCursor = text.slice(0, offset);
        const markerIndex = textToCursor.lastIndexOf('[[');
        if (markerIndex === -1) return;

        const url = buildEntityLink(entity);
        const linkNode = $createLinkNode(url, {});
        linkNode.append($createTextNode(entity.name));

        // Text before [[
        const beforeText = text.slice(0, markerIndex);
        // Text after cursor
        const afterText = text.slice(offset);

        if (beforeText) {
            const beforeNode = $createTextNode(beforeText);
            textNode.insertBefore(beforeNode);
        }

        textNode.insertBefore(linkNode);

        if (afterText) {
            const afterNode = $createTextNode(afterText);
            linkNode.insertAfter(afterNode);
            afterNode.selectStart();
        } else {
            // Insert a space after the link so cursor moves past it
            const spaceNode = $createTextNode(' ');
            linkNode.insertAfter(spaceNode);
            spaceNode.selectStart();
        }

        textNode.remove();
    });
}

/**
 * WikiLink handler class - manages the dropdown lifecycle for a single editor instance.
 */
class WikiLinkHandler {
    protected editor: LexicalEditor;
    protected context: EditorUiContext;
    protected dropdown: HTMLElement | null = null;
    protected listEl: HTMLElement | null = null;
    protected active = false;
    protected selectedIndex = 0;
    protected results: WikiLinkEntity[] = [];
    protected abortController: AbortController | null = null;

    protected searchDebounced: (query: string) => void;

    constructor(context: EditorUiContext) {
        this.context = context;
        this.editor = context.editor;
        this.searchDebounced = debounce(this.performSearch.bind(this), 200, false);
    }

    /**
     * Check if `[[` was just typed and open the dropdown if so.
     * Also update the dropdown if already active.
     */
    handleTextChange(): void {
        this.editor.getEditorState().read(() => {
            const selection = $getSelection();
            if (!$isRangeSelection(selection) || !selection.isCollapsed()) {
                this.hide();
                return;
            }

            const nodes = selection.getNodes();
            if (nodes.length === 0) {
                this.hide();
                return;
            }

            const node = nodes[0];
            if (!(node instanceof TextNode)) {
                this.hide();
                return;
            }

            const query = extractWikiQuery(node as TextNode, selection);
            if (query === null) {
                this.hide();
                return;
            }

            if (!this.active) {
                this.show();
            }

            this.searchDebounced(query);
        });
    }

    show(): void {
        if (this.active) return;
        this.active = true;
        this.abortController = new AbortController();

        this.dropdown = buildDropdown(this.context.containerDOM);
        this.listEl = this.dropdown.querySelector('.wiki-link-list') as HTMLElement;

        showLoading(this.listEl);
        positionDropdown(this.dropdown, this.context.editorDOM, this.context.containerDOM);

        this.dropdown.addEventListener('click', (event: MouseEvent) => {
            const item = (event.target as HTMLElement).closest('a[data-entity-type]') as HTMLAnchorElement | null;
            if (item) {
                event.preventDefault();
                this.selectItem(entityFromListItem(item));
            }
        }, {signal: this.abortController.signal});
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
                const empty = el('p', {class: 'text-muted px-m py-s'});
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

            if (this.dropdown) {
                positionDropdown(this.dropdown, this.context.editorDOM, this.context.containerDOM);
            }
        } catch (e) {
            if (this.active && this.listEl) {
                this.listEl.innerHTML = '';
                const err = el('p', {class: 'text-muted px-m py-s'});
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

    moveDown(): boolean {
        if (!this.active || this.results.length === 0) return false;
        this.selectedIndex = (this.selectedIndex + 1) % this.results.length;
        this.updateSelection();
        return true;
    }

    moveUp(): boolean {
        if (!this.active || this.results.length === 0) return false;
        this.selectedIndex = (this.selectedIndex - 1 + this.results.length) % this.results.length;
        this.updateSelection();
        return true;
    }

    confirmSelection(): boolean {
        if (!this.active || this.results.length === 0) return false;
        const entity = this.results[this.selectedIndex];
        if (!entity) return false;
        this.selectItem(entity);
        return true;
    }

    protected selectItem(entity: WikiLinkEntity): void {
        this.hide();
        insertWikiLink(this.editor, entity);
    }

    isActive(): boolean {
        return this.active;
    }
}

/**
 * Register wiki-link `[[` typeahead for a Lexical editor.
 */
export function registerWikiLinks(context: EditorUiContext): () => void {
    const handler = new WikiLinkHandler(context);
    const editor = context.editor;

    const unregisterUpdate = editor.registerUpdateListener(() => {
        handler.handleTextChange();
    });

    const unregisterDown = editor.registerCommand(KEY_ARROW_DOWN_COMMAND, (event: KeyboardEvent): boolean => {
        if (handler.isActive()) {
            event.preventDefault();
            handler.moveDown();
            return true;
        }
        return false;
    }, COMMAND_PRIORITY_NORMAL);

    const unregisterUp = editor.registerCommand(KEY_ARROW_UP_COMMAND, (event: KeyboardEvent): boolean => {
        if (handler.isActive()) {
            event.preventDefault();
            handler.moveUp();
            return true;
        }
        return false;
    }, COMMAND_PRIORITY_NORMAL);

    const unregisterEnter = editor.registerCommand(KEY_ENTER_COMMAND, (event: KeyboardEvent): boolean => {
        if (handler.isActive()) {
            event.preventDefault();
            event.stopPropagation();
            return handler.confirmSelection();
        }
        return false;
    }, COMMAND_PRIORITY_NORMAL);

    const unregisterEscape = editor.registerCommand(KEY_ESCAPE_COMMAND, (event: KeyboardEvent): boolean => {
        if (handler.isActive()) {
            handler.hide();
            return true;
        }
        return false;
    }, COMMAND_PRIORITY_NORMAL);

    return (): void => {
        unregisterUpdate();
        unregisterDown();
        unregisterUp();
        unregisterEnter();
        unregisterEscape();
        handler.hide();
    };
}
