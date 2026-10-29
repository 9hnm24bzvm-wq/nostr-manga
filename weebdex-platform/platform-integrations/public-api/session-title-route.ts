// Explicit draft preview reads use the normal browser account, never a signer.
import {get} from 'svelte/store';
import {Session} from '$lib/stores';
import {createSessionTitleController} from './session-title-client';
import {nativeAccountError} from './native-account-scope';
export async function publicTitleRead(id:string){
 const session=get(Session),href=window.location.href;
 if(!session?.identity?.id)throw nativeAccountError(401);
 const check=()=>{if(get(Session)!==session||window.location.href!==href)throw nativeAccountError(409);};
 const controller=await createSessionTitleController(session.identity.id,check,id);
 try{return await controller.read();}finally{controller.dispose();}
}
