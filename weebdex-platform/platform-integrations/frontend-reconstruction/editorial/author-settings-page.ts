import {env} from '$env/dynamic/public';
import {redirect} from '@sveltejs/kit';
import {publicUuid,record} from '$publicApi/validation';
export const load=async({parent}:{parent:()=>Promise<Record<string,unknown>>})=>{if(env.PUBLIC_MANGADEX_CATALOGUE==='1'){const data=await parent();redirect(307,'/catalogue-tools?kind=author&id='+publicUuid(record(data.author).id));}return{};};
