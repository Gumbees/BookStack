export interface User {
  id: number;
  name: string;
  email: string;
  role: 'admin' | 'editor' | 'viewer';
}

export interface Tag {
  name: string;
  value: string;
  order?: number;
}

interface Entity {
  id: number;
  name: string;
  slug: string;
  created_at: string;
  updated_at: string;
}

export interface Shelf extends Entity {
  description: string;
}

export interface ShelfDetails extends Shelf {
  books: Book[];
  tags: Tag[];
}

export interface Book extends Entity {
  description: string;
}

export interface PageMeta extends Entity {
  book_id: number;
  chapter_id: number | null;
  priority: number;
  draft: boolean;
  revision_count: number;
}

export interface Chapter extends Entity {
  book_id: number;
  description: string;
  priority: number;
}

export type ContentItem =
  | ({ type: 'chapter'; pages: PageMeta[] } & Chapter)
  | ({ type: 'page' } & PageMeta);

export interface BookDetails extends Book {
  tags: Tag[];
  contents: ContentItem[];
}

export interface Page extends PageMeta {
  markdown: string;
  html: string;
  tags: Tag[];
  book_slug: string;
}

export interface Revision {
  id: number;
  page_id: number;
  revision_number: number;
  name: string;
  summary: string;
  created_by: number | null;
  created_at: string;
}

export interface SearchResult {
  entity_type: 'page' | 'book' | 'chapter' | 'shelf';
  id: number;
  name: string;
  slug: string;
  book_slug: string | null;
  preview: string;
  rank: number;
}

export interface Paginated<T> {
  data: T[];
  total: number;
}
