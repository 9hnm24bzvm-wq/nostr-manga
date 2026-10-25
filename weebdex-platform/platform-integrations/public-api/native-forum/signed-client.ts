// Native catalogue discussions use the reviewed common account/request/CAS core.
// A kind/UUID/actual-thread tuple is not protocol publication or cookie authority.
import {createSignedDiscussion,type NativeDiscussionKind} from '../native-user-forum/signed-client';
import type {TitleDependencies} from '../title-client';
export type {DiscussionIntent,SignedDiscussionView,NativeDiscussionKind} from '../native-user-forum/signed-client';
export {discussionBody} from '../native-user-forum/signed-client';
export async function createSignedNativeDiscussion(kind:NativeDiscussionKind,targetId:string,threadId:string,assertCurrent=()=>{},injected:Partial<TitleDependencies>={},signal?:AbortSignal){
 return createSignedDiscussion(kind,targetId,threadId,assertCurrent,injected,signal);
}
