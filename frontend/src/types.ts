export interface OrgMembership {
  org_id: number;
  name: string;
  slug: string;
  role: 'admin' | 'editor' | 'viewer';
}

export interface AuthProvider {
  id: number;
  org_id: number | null;
  name: string;
  client_id: string;
  authorize_url: string;
  token_url: string;
  userinfo_url: string;
  scopes: string;
  enabled: boolean;
  auto_register: boolean;
}

export interface PublicProvider {
  id: number;
  name: string;
  org_id: number | null;
}

export interface OrgMemberInfo {
  user_id: number;
  name: string;
  email: string;
  role: string;
}

export interface SemanticChunk {
  content: string;
  score: number;
}

export interface SemanticResult {
  entity_type: 'page' | 'book' | 'chapter' | 'shelf';
  org_id: number;
  org_slug: string;
  id: number;
  name: string;
  slug: string;
  book_slug: string | null;
  score: number;
  chunks: SemanticChunk[];
}

export interface SemanticResponse {
  mode: string;
  results: SemanticResult[];
  stats: Record<string, unknown>;
}

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
  org_id: number;
  org_slug: string;
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
