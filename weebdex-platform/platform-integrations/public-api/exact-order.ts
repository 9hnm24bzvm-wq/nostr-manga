// Compare persisted SQL keys without rounding BIGINT/NUMERIC or sub-ms dates.
export function decimalKey(value:unknown):string{
 if(typeof value!=='string'||value.length>256||! /^-?\d+(?:\.\d+)?$/.test(value))throw Error('Invalid exact catalogue decimal key');
 return value;
}
export function compareDecimal(left:unknown,right:unknown):number{
 function parts(input:unknown){const value=decimalKey(input),negative=value[0]==='-',[whole,fraction='']=(negative?value.slice(1):value).split('.'),integer=whole.replace(/^0+(?=\d)/,'');return {negative:negative&&!/^0*$/.test(integer+fraction),integer,fraction};}
 const a=parts(left),b=parts(right);if(a.negative!==b.negative)return a.negative?-1:1;
 const sign=a.negative?-1:1;if(a.integer.length!==b.integer.length)return sign*(a.integer.length<b.integer.length?-1:1);
 if(a.integer!==b.integer)return sign*(a.integer<b.integer?-1:1);
 const size=Math.max(a.fraction.length,b.fraction.length),af=a.fraction.padEnd(size,'0'),bf=b.fraction.padEnd(size,'0');return af===bf?0:sign*(af<bf?-1:1);
}
export function utcOrderKey(value:unknown):string|null{
 if(value===null||value===undefined)return null;
 if(typeof value!=='string')throw Error('Invalid actual catalogue date');
 const match=/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.(\d{1,9}))?(?:Z|[+-]\d{2}:\d{2})$/.exec(value),time=Date.parse(value);
 if(!match||!Number.isFinite(time))throw Error('Invalid actual catalogue date');
 return new Date(time).toISOString().slice(0,19)+'.'+(match[1]??'').padEnd(9,'0')+'Z';
}
