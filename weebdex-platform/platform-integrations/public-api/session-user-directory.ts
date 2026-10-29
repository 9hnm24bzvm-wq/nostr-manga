// The ordinary directory follows the same browser session as all site actions.
import {get} from 'svelte/store';
import {Session} from '$lib/stores';
import {openSessionAccountAccess} from './session-account-access';
import {createUserDirectorySearch} from './session-user-directory-core';
import {nativeAccountError} from './native-account-scope';
export async function searchUserDirectory(input=new URLSearchParams(),signal?:AbortSignal){
 const session=get(Session);if(!session?.identity?.id)throw nativeAccountError(401);
 const check=()=>{if(signal?.aborted||session!==get(Session))throw nativeAccountError(409);};
 const account=await openSessionAccountAccess(session.identity.id,check);
 try{return await createUserDirectorySearch(async()=>account)(input,signal);}finally{account.dispose();}
}
