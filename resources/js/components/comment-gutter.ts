import {hashElement} from '../services/dom';
import {PageComments} from './page-comments';

type AlpineThis = {
    $el: HTMLElement;
    _button: HTMLButtonElement | null;
    _hovered: HTMLElement | null;
    _onOver: ((e: MouseEvent) => void) | null;
    _onOut: ((e: MouseEvent) => void) | null;
    _createButton(): HTMLButtonElement;
    _handleMouseOver(e: MouseEvent): void;
    _handleMouseOut(e: MouseEvent): void;
    _positionButton(target: HTMLElement): void;
    _triggerComment(target: HTMLElement): void;
};

/**
 * Alpine.js component data factory for the comment gutter.
 * Provides a hover-to-add affordance alongside .page-content for anchored comments.
 *
 * Pass canComment=true when the current user has CommentCreateAll permission.
 */
export function commentGutter({canComment}: {canComment: boolean}) {
    return {
        _button: null as HTMLButtonElement | null,
        _hovered: null as HTMLElement | null,
        _onOver: null as ((e: MouseEvent) => void) | null,
        _onOut: null as ((e: MouseEvent) => void) | null,

        init(this: AlpineThis) {
            if (!canComment) {
                return;
            }

            this._button = this._createButton();
            this.$el.appendChild(this._button);

            this._onOver = this._handleMouseOver.bind(this);
            this._onOut = this._handleMouseOut.bind(this);

            const content = this.$el.querySelector('.page-content') as HTMLElement | null;
            if (!content) {
                return;
            }

            content.addEventListener('mouseover', this._onOver as EventListener);
            content.addEventListener('mouseout', this._onOut as EventListener);
        },

        destroy(this: AlpineThis) {
            const content = this.$el.querySelector('.page-content') as HTMLElement | null;
            if (content && this._onOver && this._onOut) {
                content.removeEventListener('mouseover', this._onOver as EventListener);
                content.removeEventListener('mouseout', this._onOut as EventListener);
            }
        },

        _createButton(this: AlpineThis): HTMLButtonElement {
            const btn = document.createElement('button');
            btn.type = 'button';
            btn.className = 'comment-gutter-add-btn';
            btn.title = 'Add comment';
            btn.setAttribute('aria-label', 'Add comment to this section');
            btn.hidden = true;

            // SVG based on BookStack's comment icon with an added plus indicator
            btn.innerHTML = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" aria-hidden="true" fill="currentColor">
                <path d="M21.99 4c0-1.1-.89-2-1.99-2H4c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h14l4 4z" opacity="0.4"/>
                <path fill="none" d="M0 0h24v24H0z"/>
                <path d="M13 9h-2v3H8v2h3v3h2v-3h3v-2h-3z"/>
            </svg>`;

            btn.addEventListener('click', () => {
                if (this._hovered) {
                    this._triggerComment(this._hovered);
                    btn.hidden = true;
                    this._hovered = null;
                }
            });

            // Keep button visible when cursor is over it
            btn.addEventListener('mouseleave', () => {
                btn.hidden = true;
                this._hovered = null;
            });

            return btn;
        },

        _handleMouseOver(this: AlpineThis, event: MouseEvent): void {
            const target = (event.target as HTMLElement).closest('[id^="bkmrk-"]') as HTMLElement | null;
            if (!target || !this._button) {
                return;
            }

            this._hovered = target;
            this._positionButton(target);
            this._button.hidden = false;
        },

        _handleMouseOut(this: AlpineThis, event: MouseEvent): void {
            if (!this._button) {
                return;
            }

            const related = event.relatedTarget as Node | null;

            // Keep button visible if the cursor moves directly to the button
            if (related instanceof Node && this._button.contains(related)) {
                return;
            }

            // Keep visible when moving between child nodes of the same bkmrk element
            const nextTarget = related instanceof HTMLElement
                ? (related.closest('[id^="bkmrk-"]') as HTMLElement | null)
                : null;
            if (nextTarget) {
                if (nextTarget !== this._hovered) {
                    this._hovered = nextTarget;
                    this._positionButton(nextTarget);
                }
                return;
            }

            this._button.hidden = true;
            this._hovered = null;
        },

        _positionButton(this: AlpineThis, target: HTMLElement): void {
            if (!this._button) {
                return;
            }

            const container = this.$el;
            const containerBounds = container.getBoundingClientRect();
            const targetBounds = target.getBoundingClientRect();

            const relTop = (targetBounds.top - containerBounds.top) + container.scrollTop;
            const centerY = relTop + (targetBounds.height / 2);

            this._button.style.top = `${centerY}px`;
        },

        _triggerComment(this: AlpineThis, target: HTMLElement): void {
            const refId = target.id;
            const hash = hashElement(target);
            const reference = `${refId}:${hash}:`;

            const pageComments = window.$components.first('page-comments') as PageComments | null;
            if (pageComments) {
                pageComments.startNewComment(reference);
                pageComments.$el.scrollIntoView({behavior: 'smooth', block: 'nearest'});
            }
        },
    };
}
