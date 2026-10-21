// Native maintenance has one shared lease. Cancellation requests do not release
// it; the next inspector operation waits for the original promise to settle.
const pending=new WeakMap<object,Promise<void>>();
export function inspectorOperation<T>(bridge:object,operation:(operationId:string)=>Promise<T>):Promise<T>{
  const previous=pending.get(bridge)??Promise.resolve();
  const result=previous.then(()=>operation(crypto.randomUUID()));
  pending.set(bridge,result.then(()=>{},()=>{}));
  return result;
}
