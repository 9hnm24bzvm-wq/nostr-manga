import {publicUuid,record,PublicApiContractError} from '../validation';

export type NativeDiscussionKind='manga'|'chapter'|'group';
export interface NativeDiscussionTarget {kind:NativeDiscussionKind;targetId:string;threadId:string;}

export function nativeThreadId(input:unknown):string {
 if(typeof input!=='string'||!input||input.length>128||/[\\/\u0000-\u0020]/.test(input))throw new PublicApiContractError('An actual native discussion thread is required',422);
 return input;
}

// Resource routes supply both the typed public target and its actual persisted
// thread. A route UUID, native relation or signing key alone is not a target.
export function nativeDiscussionTarget(input:unknown,threadInput:unknown):NativeDiscussionTarget {
 const value=record(input,'native discussion target'),threadId=nativeThreadId(threadInput);
 if(typeof value.kind!=='string'||!['manga','chapter','group'].includes(value.kind)||value.threadId!==threadId)throw new PublicApiContractError('The resource and actual discussion thread no longer match',409);
 return Object.freeze({kind:value.kind as NativeDiscussionKind,targetId:publicUuid(value.targetId),threadId});
}
