import {env} from '$env/dynamic/public';
import {redirect} from '@sveltejs/kit';
export const load=()=>{if(env.PUBLIC_MANGADEX_CATALOGUE==='1')redirect(307,'/catalogue-tools?kind=group');return{};};
