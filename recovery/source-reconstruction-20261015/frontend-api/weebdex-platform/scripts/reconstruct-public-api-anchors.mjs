// Recovery tooling, reconstructed 2026-10-15. This is not a recovered original commit.
// Run only against the recovered attachment whose source-parity manifest is checked below.
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const frontend=path.join(root,'frontend');
const directory=path.join(root,'platform-integrations/public-api');
const sha=value=>createHash('sha256').update(value).digest('hex');
const trusted=JSON.parse(fs.readFileSync(path.join(root,'test-evidence/frontend/source-parity.json'),'utf8'));
if(trusted.schema_version!==1||trusted.file_count!==676||trusted.all_identical!==true||trusted.archive_sha256!=='845aa9bb7964cc02cc3353144c97ae42e64e09c5128e98076fa090f833f6cadd')throw Error('Unrecognized recovered attachment manifest');
const pins=Object.fromEntries(trusted.files.map(entry=>[entry.path,entry.source_sha256]));
const source=relative=>{const value=fs.readFileSync(path.join(frontend,relative),'utf8');if(!pins[relative]||sha(value)!==pins[relative])throw Error(`Attachment bytes differ: ${relative}`);return value;};
const patches=[];
function add(relative,before,after,extra={}){const original=source(relative);if(!extra.composed_before&&original.split(before).length-1!==1)throw Error(`Recovery anchor is not unique: ${relative}: ${before}`);patches.push({path:relative,sha256:pins[relative],count:1,before,after,...extra});}
const components='src/lib/components/';
const title='src/routes/(app)/title/';
const sized='`${PUBLIC_COVER_URL}/${manga.id}/${manga.relationships.cover.id}.${manga.relationships.cover.dimensions[1] < manga.relationships.cover.dimensions[0] ? "512" : "256"}.webp`';
for(const relative of [components+'MangaSearchEntryCard.svelte',components+'SearchDialogMangaEntry.svelte',components+'ChapterFeedMangaEntry.svelte','src/routes/(app)/popular/MostReadChapter.svelte',title+'Manga.svelte'])add(relative,sized,`retainedCoverUrl(manga.id, manga.relationships.cover, ${sized}, manga.relationships.cover.dimensions[1] < manga.relationships.cover.dimensions[0] ? "512" : "256")`,{helper:'media',...(relative.includes('/popular/')?{approved_public_media_exception:'uuid_cover_url_expression'}:{})});
for(const suffix of ['.512.webp','.webp']){const before='`${PUBLIC_COVER_URL}/${manga.id}/${manga.relationships.cover.id}'+suffix+'`';add(components+'MostReadCarouselEntry.svelte',before,`retainedCoverUrl(manga.id, manga.relationships.cover, ${before}, "${suffix==='.webp'?'original':'512'}")`,{helper:'media'});}
let before='`${PUBLIC_COVER_URL}/${manga.id}/${manga.relationships.cover.id}${manga.relationships.cover.ext}`';
add(title+'Manga.svelte',before,`retainedCoverUrl(manga.id, manga.relationships.cover, ${before}, "original")`,{helper:'media',approved_public_media_exception:'uuid_cover_url_expression'});
before='`${PUBLIC_COVER_URL}/${manga.id}/${v.id}${v.ext}`';add(title+'MangaCoversSection.svelte',before,`retainedCoverUrl(manga.id, v, ${before}, "original")`,{helper:'media'});
add(title+'MangaCoversSection.svelte','src="{PUBLIC_COVER_URL}/{manga.id}/{cover.id}.512.webp"','src={retainedCoverUrl(manga.id, cover, `${PUBLIC_COVER_URL}/${manga.id}/${cover.id}.512.webp`, "512")}',{helper:'media',approved_public_media_exception:'uuid_cover_url_expression'});
for(const suffix of ['${cover.ext}','.512.webp']){before='`${PUBLIC_COVER_URL}/${data.manga.id}/${cover.id}'+suffix+'`';add(components+'draft/manga/MangaEdit.svelte',before,`retainedCoverUrl(data.manga.id, cover, ${before}, "${suffix==='.512.webp'?'512':'original'}")`,{helper:'media'});}
before='`${reader.chapter.node}/data/${reader.chapter.id}/${page.data.name}`';add(components+'reader/reader-queue.svelte',before,`retainedPageUrl(reader.chapter, page.data, ${before})`,{helper:'media'});
add(title+'MangaActions.svelte','let isEmpty = $derived(!manga.relationships.available_languages || !manga.relationships.stats?.chapters);','let isEmpty = $derived(retainedMangaIsEmpty(manga, !manga.relationships.available_languages || !manga.relationships.stats?.chapters));',{helper:'presentation'});
add(title+'Manga.svelte','{#if mangaStats && comments.stats && isPublished}','{#if mangaStats && comments?.stats && isPublished}',{approved_public_availability_exception:'missing_native_comments_statistics'});
for(const relative of [components+'ChapterListEntry.svelte',components+'ChapterFeedChapterEntry.svelte']){const matches=[...source(relative).matchAll(/<a\b[^>]*?href="\/forums\/threads\/\{chapter\.relationships\.thread\.id\}"[\s\S]*?<\/a>/g)];if(matches.length!==1)throw Error(`Expected one actual chapter discussion link: ${relative}`);before=matches[0][0];add(relative,before,`{#if chapter.relationships.thread}${before}{:else}<span class="text-muted-foreground">Comments unavailable</span>{/if}`,{approved_public_availability_exception:'actual_native_forum_context_only'});}
for(const [relative,originalImport]of [[components+'reader/reader.svelte','../comments'],['src/routes/(app)/group/[id]/+layout.svelte','$lib/components/comments'],['src/routes/(app)/group/[id]/comments/+page.svelte','$lib/components/comments']])add(relative,`import * as Comments from "${originalImport}";`,'import * as Comments from "$publicApi/native-forum";');
for(const relative of ['src/routes/(app)/chapter/[id]/+page.ts','src/routes/(app)/chapter/[id]/[page]/+page.ts'])add(relative,'if (res?.body) {','if (res?.body?.relationships.thread) {');
add(components+'reader/reader.svelte','{#if reader.chapter.relationships.thread}<Comments.Comments />{/if}','{#if reader.chapter.relationships.thread}<Comments.Comments />{:else}<p role="status">Comments are unavailable for this resource.</p>{/if}',{composed_before:true,approved_public_availability_exception:'missing_native_forum_context'});
for(const family of ['Manga','Chapter','Cover','Author','Group','Tag','Statistics','Analytics'])source(`src/lib/API/${family}.ts`);
const paths=[...new Set([...patches.map(entry=>entry.path),...['Manga','Chapter','Cover','Author','Group','Tag','Statistics','Analytics'].map(family=>`src/lib/API/${family}.ts`)])].sort();
fs.writeFileSync(path.join(directory,'source-hashes.json'),JSON.stringify(Object.fromEntries(paths.map(relative=>[relative,pins[relative]])),null,2)+'\n');
fs.writeFileSync(path.join(directory,'anchors.json'),JSON.stringify(patches,null,2)+'\n');
console.log(JSON.stringify({reconstructed:true,source:'attachment source-parity manifest',paths:paths.length,anchors:patches.length}));
