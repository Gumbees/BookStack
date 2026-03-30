# Private Notebooks

Private notebooks are personal books that are only visible to their owner. They are ordinary `Book` entities with the `is_private` flag set to `true`.

## Auto-creation via /my-notebook

Visiting `/my-notebook` while logged in automatically creates a private notebook for the current user if one doesn't already exist, then redirects to it. The book is named `{User}'s Notebook`.

This is the recommended entry point for users who want a private scratchpad without having to create and configure a book manually.

## Permission model

Private books bypass the standard joint permission system entirely.

- Only the owning user (`owned_by`) can see or access the book and all its children (chapters, pages)
- Admins do not automatically get access; the ownership check is the sole gate
- The book never receives joint permission rows, so granting entity permissions to other users or roles has no effect on private books
- Even if a user somehow constructs a direct URL to a page inside a private book they don't own, access is denied

This is enforced in `PermissionApplicator` by checking `is_private = true AND owned_by = {currentUserId}` as a special case before joint permission evaluation.

## `is_private` flag

The flag lives on the `books` table (`is_private` boolean, default `false`). It is also cast to boolean in the `Book` model so it behaves consistently in PHP.

Only the `PrivateNotebookService` sets this flag during book creation. There is no UI for toggling it on arbitrary books; it is exclusively set on books created through the auto-notebook flow.

## Search exclusion

Private books owned by other users are excluded from all search results and entity listings via the visibility scope in `PermissionApplicator`. A user will never see another user's private content appear in search, recent pages, or entity selectors.

## Ownership

The `owned_by`, `created_by`, and `updated_by` fields on the book are all set to the creating user's ID. The slug is generated normally and must be unique.

## Implementation files

| File | Role |
|------|------|
| `app/Entities/Services/PrivateNotebookService.php` | Creates the private book for a user |
| `app/Entities/Controllers/PrivateNotebookController.php` | Handles `GET /my-notebook` and redirects |
| `app/Permissions/PermissionApplicator.php` | Enforces private visibility in queries |
| `app/Entities/Models/Book.php` | Declares the `is_private` boolean cast |
