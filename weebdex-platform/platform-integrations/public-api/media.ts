import { coverUrl } from './normalize';
import { PublicApiContractError } from './validation';
import type { CoverView } from './types';
export function retainedCoverUrl(mangaId:string,cover:unknown,nativeUrl:string,rendition:'original'|'256'|'512'='original'):string {if(!cover||typeof cover!=='object'||!('public_api'in cover))return nativeUrl;return coverUrl('/covers',mangaId,cover as CoverView,rendition);}
export function retainedPageUrl(chapter:unknown,page:unknown,nativeUrl:string):string {if(!chapter||typeof chapter!=='object'||!('public_api'in chapter))return nativeUrl;if(!page||typeof page!=='object'||!('public_url'in page)||typeof page.public_url!=='string')throw new PublicApiContractError('The public reader page has no at-home delivery URL');return page.public_url;}
