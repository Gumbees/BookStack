# Scratch Notes

Scratch notes are short collaborative notes attached to a page. They are separate from the page content and visible to anyone with view access to the page. Multiple users can add and see each other's scratch notes in real time (via a page reload or live panel).

Scratch notes are intended for quick asides, review comments, or in-progress thoughts that shouldn't go in the main page content.

## Permission model

| Action | Required permission |
|--------|-------------------|
| Read scratch notes | View permission on the page |
| Add a scratch note | Edit (`page-update`) permission on the page |
| Edit own scratch note | Edit permission on the page + note authorship |
| Edit another user's note | Admin role |
| Delete own scratch note | Edit permission on the page + note authorship |
| Delete another user's note | Admin role |

Guest access is blocked on all scratch note endpoints.

## Attribution

Each note records the `user_id` of the author. The `user_name` field is returned in all responses so the UI can display who wrote each note without a separate lookup.

## @mentions

Type `@Name` in a scratch note to mention another user by their display name. Mentions trigger an in-app notification to the mentioned user. Deduplication prevents the same user being notified twice for the same note across edits.

Mention processing is gated on the `notifications.mention_enabled` and `notifications.in_app_enabled` admin settings. If either is `false`, mentions in scratch notes are silently ignored.

## Web endpoints

These endpoints are used by the page view UI. They require an authenticated (non-guest) session.

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/ajax/page/{pageId}/scratch-notes` | List all notes for a page |
| `POST` | `/ajax/page/{pageId}/scratch-notes` | Create a new note |
| `PUT` | `/ajax/page/{pageId}/scratch-notes/{noteId}` | Update a note |
| `DELETE` | `/ajax/page/{pageId}/scratch-notes/{noteId}` | Delete a note |

## API endpoints

The REST API exposes scratch notes under the page resource.

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `api/pages/{id}/scratch-notes` | List all notes for a page |
| `POST` | `api/pages/{id}/scratch-notes` | Create a note |
| `PUT` | `api/pages/{id}/scratch-notes/{noteId}` | Update a note |
| `DELETE` | `api/pages/{id}/scratch-notes/{noteId}` | Delete a note |

### Request body (create/update)

```json
{
  "content": "Check the figures in section 3. @Alice can you verify?"
}
```

Content is limited to 2000 characters.

### Response object

```json
{
  "id": 7,
  "page_id": 42,
  "user_id": 3,
  "user_name": "Bob Smith",
  "content": "Check the figures in section 3. @Alice can you verify?",
  "created_at": "2026-03-15T14:30:00+00:00",
  "updated_at": "2026-03-15T14:35:00+00:00"
}
```

The list endpoints wrap this in a standard `data`/`total` envelope:

```json
{
  "data": [...],
  "total": 3
}
```
