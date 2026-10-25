// Timing out an await never pretends that its underlying operation was cancelled.
export async function disposeLateLauncher(value,markForced){
  try{await value.close();}catch{markForced();await value.kill();}
}
export function createOwnedWork(deadline,aggregate=new Set()) {
  let open=true,disposalFailed=false;const pending=new Set(),waiters=new Set();
  const check=()=>{if(!open||Date.now()>=deadline)throw Error('Owned work closed');};
  const track=promise=>{pending.add(promise);aggregate.add(promise);const done=()=>{pending.delete(promise);aggregate.delete(promise);};promise.then(done,done);return promise;};
  async function operation(action,{capture,dispose}={}) {
    check();
    const task=track(Promise.resolve().then(()=>{check();return action();}).then(async value=>{
      const release=async()=>{if(dispose)try{await dispose(value);}catch(error){disposalFailed=true;throw error;}};
      if(!open||Date.now()>=deadline){await release();throw Error('Late owned result disposed');}
      try{capture?.(value);}catch(error){await release();throw error;}return value;
    }));
    let timer,rejectWait;
    try{return await Promise.race([task,new Promise((_,reject)=>{rejectWait=reject;waiters.add(reject);timer=setTimeout(()=>reject(Error('Owned deadline')),Math.max(0,deadline-Date.now()));})]);}
    finally{clearTimeout(timer);waiters.delete(rejectWait);}
  }
  function cancel(){open=false;for(const reject of waiters)reject(Error('Owned work cancelled'));waiters.clear();}
  return {operation,track,cancel,pending:()=>pending.size,unsafe:()=>disposalFailed,settlement:()=>Promise.allSettled([...pending])};
}
