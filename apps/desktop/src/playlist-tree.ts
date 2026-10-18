import type {PlaylistNode} from "./playlist-types";

export function playlistIndex(nodes:PlaylistNode[]){
  const byId=new Map(nodes.map(node=>[node.id,node]));
  const children=new Map<string|null,PlaylistNode[]>();
  for(const node of nodes){if(node.deleted)continue;const siblings=children.get(node.parent_id)??[];siblings.push(node);children.set(node.parent_id,siblings);}
  for(const siblings of children.values())siblings.sort((a,b)=>a.position-b.position||a.id.localeCompare(b.id));
  return {byId,children};
}
export function descendantIds(nodes:PlaylistNode[],id:string){
  const children=new Map<string, string[]>();
  for(const node of nodes){if(node.parent_id){const list=children.get(node.parent_id)??[];list.push(node.id);children.set(node.parent_id,list);}}
  const ids=new Set<string>(),pending=[id];
  while(pending.length){const next=pending.pop()!;if(ids.has(next))continue;ids.add(next);pending.push(...children.get(next)??[]);}
  return ids;
}
export function folderOptions(nodes:PlaylistNode[],excluding:string|null){
  const omitted=excluding?descendantIds(nodes,excluding):new Set<string>();
  const byId=new Map(nodes.map(n=>[n.id,n]));
  return nodes.filter(n=>!n.deleted&&n.kind==="folder"&&!omitted.has(n.id)).map(node=>{
    const parts=[node.name],seen=new Set([node.id]);let parent=node.parent_id;
    while(parent&&!seen.has(parent)){seen.add(parent);const ancestor=byId.get(parent);if(!ancestor)break;parts.unshift(ancestor.name);parent=ancestor.parent_id;}
    return {id:node.id,label:parts.join(" / ")};
  }).sort((a,b)=>a.label.localeCompare(b.label)||a.id.localeCompare(b.id));
}
export function visiblePlaylistNodes(nodes:PlaylistNode[],expanded:Set<string>){
  const index=playlistIndex(nodes),out:{node:PlaylistNode;depth:number}[]=[],seen=new Set<string>();
  const pending=(index.children.get(null)??[]).map(node=>({node,depth:0})).reverse();
  while(pending.length){const entry=pending.pop()!;if(seen.has(entry.node.id))continue;seen.add(entry.node.id);out.push(entry);if(entry.node.kind==="folder"&&expanded.has(entry.node.id))for(const child of [...index.children.get(entry.node.id)??[]].reverse())pending.push({node:child,depth:entry.depth+1});}
  return out;
}
