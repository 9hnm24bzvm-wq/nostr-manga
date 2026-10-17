// Reconstructed from retained complete wire/view declarations; unverified.
export type PublicUuid = string & { readonly __publicUuid: unique symbol };
export type ResourceType = 'manga_relation' | 'manga_recommendation' | 'manga' | 'chapter' | 'cover_art' | 'author' | 'artist' | 'scanlation_group' | 'tag' | 'user' | 'creator' | 'custom_list';
export interface Relationship { id: PublicUuid; type: ResourceType; related?: string; attributes?: Record<string, unknown>; }
export interface Resource<A extends Record<string, unknown> = Record<string, unknown>> extends Relationship { attributes: A; relationships: Relationship[]; }
export interface Entity<A extends Record<string, unknown> = Record<string, unknown>> { result: 'ok'; response: 'entity'; data: Resource<A>; }
export interface Collection<A extends Record<string, unknown> = Record<string, unknown>> { result: 'ok'; response: 'collection'; data: Resource<A>[]; limit: number; offset: number; total: number; }
export interface ApiErrorItem { id: PublicUuid; status: number; title: string; detail?: string; context?: string; }
export interface ApiError { result: 'error'; errors: ApiErrorItem[]; }
export interface Response<T> { status: number; body?: T; headers?: Record<string, string | null>; }
export interface StatisticsEnvelope { result: 'ok'; statistics: Record<string, Record<string, unknown>>; }
export type NativeForumKind = 'manga' | 'chapter' | 'group';
export interface NativeForumResource { kind: NativeForumKind; id: string; }
export interface NativeForumEnvelope { result:'ok'; data:(NativeForumResource & {thread:Record<string,unknown>|null})[]; unavailable:NativeForumResource[]; }
export interface AtHome { result: 'ok'; baseUrl: string; chapter: { hash: string; data: string[]; dataSaver: string[] }; nostr?: { originalsOnly?: boolean; pageDimensions?: number[][]; dataSaverDimensions?: number[][] }; }
export interface Aggregate { result: 'ok'; volumes: Record<string, { volume: string | null; count: number; chapters: Record<string, { chapter: string | null; id: PublicUuid; others: PublicUuid[]; count: number }> }>; }
export interface PublicPage { name: string; dimensions: number[]; public_url: string; }
export interface ViewProvenance { resource: Resource; edge?: { owner: PublicUuid; resource: Resource }; chapter_count?: number; native_forum?: {status:number; available:boolean}; }
export interface CoverView { id: PublicUuid; file_name: string; ext: string; dimensions: number[]; volume: string | null; description?: string; language: string; version: number; public_api: ViewProvenance; }
export interface NamedView { id: PublicUuid; name: string; description?: string; version: number; public_api: ViewProvenance; [key: string]: unknown; }
export interface TagView { id: PublicUuid; name: string; group: string; public_api: ViewProvenance; }
export interface MangaView { id: PublicUuid; title: string; alt_titles: Record<string, string[]>; description?: string; year: number | null; language: string; demographic: string | null; status: string; content_rating: string; last_volume: string | null; last_chapter: string | null; state: string; locked: boolean; version: number; created_at?: string; updated_at?: string; public_api: ViewProvenance; relationships: { cover?: CoverView; tags: TagView[]; authors: NamedView[]; artists: NamedView[]; links: Record<string, string>; available_languages?: string[]; relations?: MangaView[]; [key: string]: unknown; }; }
export interface ChapterView { id: PublicUuid; title: string | null; volume: string | null; chapter: string | null; language: string; version: number; created_at?: string; updated_at?: string; published_at?: string; is_unavailable: boolean; external_url: string | null; node?: string; data?: PublicPage[]; data_optimized?: PublicPage[]; public_api: ViewProvenance; relationships: { manga: MangaView; groups: NamedView[]; uploader?: NamedView; [key: string]: unknown; }; }
export interface ViewFeed<T> { total: number; limit: number; page: number; offset: number; data: T[]; map?: { manga: Record<string, MangaView> }; }
