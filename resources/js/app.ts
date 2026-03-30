import {EventManager} from './services/events';
import {HttpManager} from './services/http';
import {Translator} from './services/translations';
import * as componentMap from './components/index';
import {ComponentStore} from './services/components';
import {baseUrl, importVersioned} from "./services/util";
import Alpine from 'alpinejs';
import {scratchNotes} from './components/scratch-notes';
import {commentGutter} from './components/comment-gutter';
import {collaborativeEditor} from './components/collaborative-editor';
import {collabPresence} from './components/collab-presence';
import {backlinksPanel} from './components/backlinks-panel';

// eslint-disable-next-line no-underscore-dangle
window.__DEV__ = false;

// Make common important util functions global
window.baseUrl = baseUrl;
window.importVersioned = importVersioned;

// Setup events, http & translation services
window.$http = new HttpManager();
window.$events = new EventManager();
window.$trans = new Translator();

// Load & initialise components
window.$components = new ComponentStore();
window.$components.register(componentMap);
window.$components.init();

// Initialise Alpine.js after BookStack's own component system.
// Alpine uses x-data attributes so it does not conflict with the
// BookStack component system which uses the 'component' attribute.
Alpine.data('scratchNotes', scratchNotes);
Alpine.data('commentGutter', commentGutter);
Alpine.data('collaborativeEditor', collaborativeEditor);
Alpine.data('collabPresence', collabPresence);
Alpine.data('backlinksPanel', backlinksPanel);
Alpine.start();
