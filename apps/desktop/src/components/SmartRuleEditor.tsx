import {useEffect,useRef,useState} from "react";
import {Plus,X,ArrowUp,ArrowDown} from "lucide-react";
import type {Bridge} from "../types";
import type {SmartCatalog,SmartDefinition,SmartRule,RuleValue,PlaylistNode} from "../playlist-types";
import {allowedOperators,operators,initialRuleValue,calendarDay} from "../smart-rules";
import type {SmartOperator} from "../smart-rules";
import {errorMessage} from "../api";

function DateInput({label,value,catalog,bridge,onValue,onPending,onError}:{label:string;value:number;catalog:SmartCatalog;bridge:Bridge;onValue:(v:number)=>void;onPending:(delta:number)=>void;onError:(message:string)=>void}){
  const [text,setText]=useState(()=>Number.isFinite(value)?calendarDay(value,catalog.time_zone):"");
  const request=useRef(0),releasePending=useRef<(()=>void)|null>(null);
  useEffect(()=>{if(Number.isFinite(value))setText(calendarDay(value,catalog.time_zone));},[value,catalog.time_zone]);
  useEffect(()=>()=>{request.current++;releasePending.current?.();},[bridge,catalog.time_zone,catalog.stamp.preference_revision]);
  return <input type="date" aria-label={label} value={text} onChange={event=>{
    const day=event.target.value;setText(day);const generation=++request.current;
    releasePending.current?.();
    // Keep the definition invalid until this exact calendar input resolves.
    onValue(NaN);
    if(!day){onError("Choose a calendar date");return;}
    onPending(1);
    const finish=()=>{if(releasePending.current===finish){releasePending.current=null;onPending(-1);}};
    releasePending.current=finish;
    void bridge.resolvePlaylistDate(day,catalog.stamp.preference_revision).then(next=>{if(generation===request.current)onValue(next);}).catch(error=>{if(generation===request.current)onError(errorMessage(error));}).finally(finish);
  }}/>;
}
function path(node:PlaylistNode,nodes:PlaylistNode[]):string {
  const names=[node.name],seen=new Set([node.id]);let parent=node.parent_id;
  while(parent&&!seen.has(parent)){seen.add(parent);const found=nodes.find(n=>n.id===parent);if(!found)break;names.unshift(found.name);parent=found.parent_id;}
  return names.join(" / ");
}
type ValueUpdate=RuleValue|((current:RuleValue)=>RuleValue);
function ValueInput({rule,index,catalog,bridge,onChange,onPending,onError}:{rule:SmartRule;index:number;catalog:SmartCatalog;bridge:Bridge;onChange:(value:ValueUpdate)=>void;onPending:(delta:number)=>void;onError:(message:string)=>void}){
  const [search,setSearch]=useState("");const value=rule.value,n=index+1;
  const number=(text:string)=>text.trim()?Number(text):NaN;
  switch(value.kind){
    case "none":return null;
    case "text":return rule.operator==="KeySimilar"?<select aria-label={`Key for condition ${n}`} value={value.value} onChange={e=>onChange({kind:"text",value:e.target.value})}>{Array.from({length:12},(_,i)=>[`${i+1}A`,`${i+1}B`]).flat().map(key=><option key={key}>{key}</option>)}</select>:<input aria-label={`Value for condition ${n}`} value={value.value} maxLength={rule.operator.includes("RegExp")?100:1048576} placeholder={rule.operator.includes("RegExp")?"/pattern/flags":"Text to match"} onChange={e=>onChange({kind:"text",value:e.target.value})}/>;
    case "number":return <input type="number" step="any" aria-label={`Number for condition ${n}`} value={Number.isFinite(value.value)?value.value:""} onChange={e=>onChange({kind:"number",value:number(e.target.value)})}/>;
    case "number_range":return <div className="rule-range"><input type="number" step="any" aria-label={`Minimum for condition ${n}`} value={Number.isFinite(value.value.start)?value.value.start:""} onChange={e=>onChange({kind:"number_range",value:{...value.value,start:number(e.target.value)}})}/><span>to</span><input type="number" step="any" aria-label={`Maximum for condition ${n}`} value={Number.isFinite(value.value.end)?value.value.end:""} onChange={e=>onChange({kind:"number_range",value:{...value.value,end:number(e.target.value)}})}/></div>;
    case "date":return <DateInput label={`Date for condition ${n}`} value={value.value} catalog={catalog} bridge={bridge} onValue={v=>onChange({kind:"date",value:v})} onPending={onPending} onError={onError}/>;
    case "date_range":return <div className="rule-range"><DateInput label={`Start date for condition ${n}`} value={value.value.start} catalog={catalog} bridge={bridge} onValue={v=>onChange(current=>current.kind==="date_range"?{kind:"date_range",value:{...current.value,start:v}}:current)} onPending={onPending} onError={onError}/><span>to</span><DateInput label={`End date for condition ${n}`} value={value.value.end} catalog={catalog} bridge={bridge} onValue={v=>onChange(current=>current.kind==="date_range"?{kind:"date_range",value:{...current.value,end:v}}:current)} onPending={onPending} onError={onError}/></div>;
    case "relative":return <div className="rule-range"><input type="number" step="1" min={-1000000} max={1000000} aria-label={`Amount for condition ${n}`} value={Number.isFinite(value.value.count)?value.value.count:""} onChange={e=>onChange({kind:"relative",value:{...value.value,count:number(e.target.value)}})}/><select aria-label={`Calendar unit for condition ${n}`} value={value.value.unit} onChange={e=>onChange({kind:"relative",value:{...value.value,unit:e.target.value as "days"|"weeks"|"months"}})}><option value="days">days</option><option value="weeks">weeks</option><option value="months">months</option></select></div>;
    case "color":return <><input aria-label={`Color for condition ${n}`} list="smart-rule-colors" value={value.value??""} placeholder="None, stored name or #hex" maxLength={128} onChange={e=>onChange({kind:"color",value:e.target.value||null})}/><datalist id="smart-rule-colors">{["Red","Blue","Orange","Yellow","Green","Magenta","Purple"].map(color=><option key={color} value={color}/>)}</datalist></>;
    case "playlist":{
      const choices=catalog.playlists.filter(p=>!p.deleted&&p.kind===value.value.kind);
      return <select aria-label={`Referenced playlist for condition ${n}`} value={value.value.id} onChange={e=>onChange({kind:"playlist",value:{...value.value,id:e.target.value,path:[]}})}><option value="">Choose a {value.value.kind==="folder"?"folder":"playlist"}</option>{value.value.id&&!choices.some(n=>n.id===value.value.id)?<option value={value.value.id} disabled>Unavailable playlist</option>:null}{choices.map(node=><option key={node.id} value={node.id}>{path(node,catalog.playlists)}</option>)}</select>;
    }
    case "ids":{
      const categories=operators[rule.operator].group==="categories";
      const choices=categories?catalog.categories:catalog.tags;
      const filtered=choices.filter(c=>c.name.toLowerCase().includes(search.toLowerCase()));
      return <div className="rule-choices"><input type="search" aria-label={`${categories?"Category":"Tag"} search for condition ${n}`} placeholder={categories?"Find categories":"Find tags"} value={search} onChange={e=>setSearch(e.target.value)}/><div>{filtered.slice(0,100).map(choice=><label key={choice.id}><input type="checkbox" checked={value.value.includes(choice.id)} onChange={e=>onChange({kind:"ids",value:e.target.checked?[...value.value,choice.id]:value.value.filter(id=>id!==choice.id)})}/>{choice.name}</label>)}</div>{filtered.length>100?<small>Showing 100 of {filtered.length}. Refine the search.</small>:null}{value.value.filter(id=>!choices.some(c=>c.id===id)).map(id=><span className="attention" key={id}>Unavailable selection: {id}</span>)}</div>;
    }
  }
}
export default function SmartRuleEditor({bridge,catalog,value,onChange,onPending,onError}:{bridge:Bridge;catalog:SmartCatalog;value:SmartDefinition;onChange:(definition:SmartDefinition)=>void;onPending:(delta:number)=>void;onError:(message:string)=>void}){
  const fields=Object.entries(catalog.catalog.fields).sort(([,a],[,b])=>a.label.localeCompare(b.label));
  const latest=useRef(value);latest.current=value;
  const update=(id:string,change:Partial<SmartRule>)=>{const next={...latest.current,rules:latest.current.rules.map(r=>r.id===id?{...r,...change}:r)};latest.current=next;onChange(next);};
  const updateValue=(id:string,next:ValueUpdate)=>{const current=latest.current.rules.find(r=>r.id===id);if(current)update(id,{value:typeof next==="function"?next(current.value):next});};
  const move=(index:number,by:number)=>{const rules=[...value.rules];[rules[index],rules[index+by]]=[rules[index+by],rules[index]];onChange({...value,rules});};
  return <section className="smart-rules" aria-label="Smart playlist conditions">
    <div className="smart-match"><span>Match</span><select aria-label="Match conditions" value={value.match_all?"all":"any"} onChange={e=>onChange({...value,match_all:e.target.value==="all"})}><option value="all">all conditions</option><option value="any">any condition</option></select><span className="muted">{value.rules.length}/128</span></div>
    {value.rules.map((rule,index)=>{
      const field=catalog.catalog.fields[rule.field],allowed=allowedOperators(rule.field,field?.kind??"");
      return <fieldset className="smart-condition" key={rule.id}><legend>Condition {index+1}</legend>
        <div className="smart-condition-head"><select aria-label={`Field for condition ${index+1}`} value={rule.field} onChange={e=>{const field=e.target.value;const list=allowedOperators(field,catalog.catalog.fields[field].kind);const operator=list[0];update(rule.id,{field,operator,value:initialRuleValue(operator)});}}>{!field?<option value={rule.field} disabled>Unavailable field: {rule.field}</option>:null}{fields.map(([id,f])=><option key={id} value={id} disabled={!!f.unavailable_reason||!allowedOperators(id,f.kind).length}>{f.label}{f.unavailable_reason?" · unavailable":""}</option>)}</select>
          <select aria-label={`Operator for condition ${index+1}`} value={rule.operator} onChange={e=>{const operator=e.target.value as SmartOperator;update(rule.id,{operator,value:initialRuleValue(operator)});}}>{!allowed.includes(rule.operator)?<option value={rule.operator} disabled>{operators[rule.operator].label} · unavailable</option>:null}{allowed.map(id=><option key={id} value={id}>{operators[id].label}</option>)}</select>
          <button type="button" aria-label={`Move condition ${index+1} up`} disabled={index===0} onClick={()=>move(index,-1)}><ArrowUp size={13}/></button><button type="button" aria-label={`Move condition ${index+1} down`} disabled={index===value.rules.length-1} onClick={()=>move(index,1)}><ArrowDown size={13}/></button><button type="button" aria-label={`Remove condition ${index+1}`} onClick={()=>onChange({...value,rules:value.rules.filter(r=>r.id!==rule.id)})}><X size={14}/></button>
        </div>
        {field?.unavailable_reason?<p className="attention">{field.unavailable_reason}</p>:null}
        <div className="smart-condition-value"><ValueInput key={`${rule.field}:${rule.operator}`} rule={rule} index={index} catalog={catalog} bridge={bridge} onChange={next=>updateValue(rule.id,next)} onPending={onPending} onError={onError}/></div>
        <label className="rule-or"><input type="checkbox" checked={rule.or} onChange={e=>update(rule.id,{or:e.target.checked})} aria-label={`OR group for condition ${index+1}`}/>OR group</label>
      </fieldset>;
    })}
    {value.match_all&&value.rules.some(r=>r.or)?<p className="muted">Two or more flagged conditions form one OR group; all unflagged conditions still apply.</p>:null}
    <button type="button" className="secondary" disabled={value.rules.length>=128||!fields.some(([id,f])=>!f.unavailable_reason&&allowedOperators(id,f.kind).length)} onClick={()=>{const field=fields.find(([id,f])=>id==="title"&&!f.unavailable_reason)?.[0]??fields.find(([id,f])=>!f.unavailable_reason&&allowedOperators(id,f.kind).length)?.[0];if(!field)return;const operator=allowedOperators(field,catalog.catalog.fields[field].kind)[0];onChange({...value,rules:[...value.rules,{id:crypto.randomUUID(),field,operator,value:initialRuleValue(operator),or:false}]});}}><Plus size={14}/>Add condition</button>
  </section>;
}
