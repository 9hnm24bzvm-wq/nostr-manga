// A public unclaimed label needs the actual typed resource and its relationships.
// The gate-on Claim dialog only navigates to the signer-bound, versioned review
// command; the native cookie/report-ID submission component is never selected.
export function retainedGroupCanClaim(group:unknown,nativeUnclaimed:boolean):boolean {
 if(!group||typeof group!=='object')return false;
 if(!('public_api' in group))return nativeUnclaimed;
 const wrapper=group.public_api;if(!wrapper||typeof wrapper!=='object'||!('resource'in wrapper))return false;
 const row=wrapper.resource;if(!row||typeof row!=='object'||!('type'in row)||row.type!=='scanlation_group'||!('attributes'in row)||!row.attributes||typeof row.attributes!=='object'||!('relationships'in row)||!Array.isArray(row.relationships))return false;
 return !('locked'in row.attributes&&row.attributes.locked===true)&&!row.relationships.some((item:unknown)=>item&&typeof item==='object'&&'type'in item&&item.type==='leader');
}
