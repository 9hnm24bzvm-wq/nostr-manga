// Reviewed-run acquisition only. Never launches or installs a browser.
import fs from 'node:fs';
import path from 'node:path';
import https from 'node:https';
import {createHash} from 'node:crypto';
import {fileURLToPath,pathToFileURL} from 'node:url';
const repo=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../../..');
const insist=v=>{if(!v)throw Error('Browser provenance check failed');};
const table=Uint32Array.from({length:256},(_,n)=>{let value=n;for(let i=0;i<8;i++)value=(value>>>1)^((value&1)?0x82f63b78:0);return value>>>0;});
export function crc32c(bytes,state=0xffffffff){for(const byte of bytes)state=(state>>>8)^table[(state^byte)&255];return state>>>0;}
const finalCrc=value=>{const b=Buffer.alloc(4);b.writeUInt32BE((value^0xffffffff)>>>0);return b.toString('base64');};
function response(url){return new Promise((resolve,reject)=>{const request=https.get(url,{timeout:30000},res=>{if(res.statusCode!==200){res.resume();reject(Error('Official download unavailable'));return;}resolve(res);});request.once('timeout',()=>request.destroy(Error('Acquisition timeout')));request.once('error',reject);});}
async function metadata(pin){const url='https://storage.googleapis.com/storage/v1/b/chrome-for-testing-public/o/'+encodeURIComponent(pin.version+'/linux64/chrome-linux64.zip')+'?generation='+pin.generation+'&fields=name,generation,size,md5Hash,crc32c';const res=await response(url),chunks=[];let size=0;for await(const chunk of res){size+=chunk.length;if(size>65536){res.destroy();throw Error('Metadata cap');}chunks.push(chunk);}const data=JSON.parse(Buffer.concat(chunks).toString('utf8'));insist(data.generation===pin.generation&&data.name===pin.version+'/linux64/chrome-linux64.zip'&&Number(data.size)===pin.size&&data.md5Hash===pin.md5Base64&&data.crc32c===pin.crc32cBase64);}
export async function acquire(destination){insist(process.arch==='x64'&&process.platform==='linux');const pins=JSON.parse(fs.readFileSync(path.join(repo,'.ci/native-upload-counterfactual/browser-pins.json'),'utf8'));insist(pins.browsers.length===2);fs.mkdirSync(destination,{recursive:false});const result=[];
  for(const pin of pins.browsers){insist(pin.kind==='full-chrome'&&pin.platform==='linux64'&&/^https:\/\/storage\.googleapis\.com\/chrome-for-testing-public\/[0-9.]+\/linux64\/chrome-linux64\.zip$/.test(pin.download)&&pin.size<=256*1024*1024);await metadata(pin);const res=await response(pin.download+'?generation='+pin.generation);insist(res.headers['x-goog-generation']===pin.generation&&Number(res.headers['content-length'])===pin.size);const filename=path.join(destination,pin.version+'.zip'),file=fs.openSync(filename,'wx');const md5=createHash('md5'),sha=createHash('sha256');let size=0,crc=0xffffffff;
    try{for await(const chunk of res){size+=chunk.length;insist(size<=pin.size);md5.update(chunk);sha.update(chunk);crc=crc32c(chunk,crc);let offset=0;while(offset<chunk.length)offset+=fs.writeSync(file,chunk,offset);}}finally{fs.closeSync(file);res.destroy();}
    insist(size===pin.size&&md5.digest('base64')===pin.md5Base64&&finalCrc(crc)===pin.crc32cBase64);result.push({...pin,sha256:sha.digest('hex')});
  }
  // Immutable acquisition witness precedes extraction or executable use.
  const target=path.join(destination,'download-manifest.json');fs.writeFileSync(target,JSON.stringify(result)+'\n',{flag:'wx',mode:0o444});return result;
}
if(import.meta.url===pathToFileURL(process.argv[1]??'').href){try{insist(process.argv.length===3);await acquire(path.resolve(process.argv[2]));console.log('Official browser acquisition verified');}catch{console.error('Official browser acquisition failed');process.exitCode=1;}}
