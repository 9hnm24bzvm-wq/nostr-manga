// List order is changed in one scope-bound device transaction, never by title text.
import { DeviceStore, normalizeDevice, type DeviceRecord } from './device-store';
import { request } from './protocol/indexeddb.js';
export const orderedListMembers = (records: readonly DeviceRecord[], list: string) => records.filter(row => row.family === 'members' && row.list === list).sort((a,b) => (a.position! - b.position!) || a.key.localeCompare(b.key));
export async function editDeviceListMember(device: DeviceStore, list: string, series: string, direction: -1 | 0 | 1, assertCurrent = () => {}) {
 if (![-1,0,1].includes(direction)) throw Error('Invalid list move');
 assertCurrent();
 return device.mutate(async rows => {
  assertCurrent();
  const records = await request(rows.getAll()) as DeviceRecord[];
  assertCurrent();
  if (!records.some(row=>row.family==='lists' && row.id===list) || !records.some(row=>row.family==='series' && row.id===series)) throw Error('Choose a saved title and an existing list');
  const members = orderedListMembers(records,list), index=members.findIndex(row=>row.series===series);
  if (direction === 0) {
   if (index >= 0) return;
   members.push(normalizeDevice({family:'members',key:list+':'+series,list,series,position:members.length,updated_at:Math.floor(Date.now()/1000)}));
  } else {
   if (index < 0) throw Error('This title is no longer in the list');
   const next=index+direction; if(next<0||next>=members.length)return;
   [members[index],members[next]]=[members[next],members[index]];
  }
  members.forEach((row,position)=>{if(row.position!==position||!records.some(old=>old.family==='members'&&old.key===row.key))rows.put(normalizeDevice({...row,position,updated_at:Math.max(Math.floor(Date.now()/1000),row.updated_at+1)}));});
  assertCurrent();
 });
}
