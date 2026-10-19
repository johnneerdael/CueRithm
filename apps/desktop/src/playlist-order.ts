export function moveSelectedPlaylistSlots(ids:string[],selected:ReadonlySet<string>,direction:-1|1){
 const next=[...ids];
 if(direction===-1){for(let i=1;i<next.length;i++)if(selected.has(next[i])&&!selected.has(next[i-1]))[next[i-1],next[i]]=[next[i],next[i-1]];}
 else {for(let i=next.length-2;i>=0;i--)if(selected.has(next[i])&&!selected.has(next[i+1]))[next[i],next[i+1]]=[next[i+1],next[i]];}
 return next;
}

export function insertPlaylistSelectionBefore(ids:string[],selected:ReadonlySet<string>,target:string){
 if(selected.has(target))return ids;
 const moving=ids.filter(id=>selected.has(id)),remaining=ids.filter(id=>!selected.has(id)),position=remaining.indexOf(target);
 if(position<0)throw Error("Drop target is no longer in this track list.");
 return [...remaining.slice(0,position),...moving,...remaining.slice(position)];
}
