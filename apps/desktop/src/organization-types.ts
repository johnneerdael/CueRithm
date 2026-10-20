export interface OrganizationSettings {
  mode:"leave_in_place"|"rename_in_place"|"move"|"copy";
  pattern:string;
  missing:"block"|"omit_optional"|"skip_optional";
  organization_optional:boolean;
  destination_root_id:string|null;
  collision:"block"|"skip";
}
export const defaultOrganization=():OrganizationSettings=>({mode:"leave_in_place",pattern:"{artist} - {title}",missing:"block",organization_optional:false,destination_root_id:null,collision:"block"});
export interface OrganizationRoot {id:string;display_path:string}
export type OrganizationScope={kind:"tracks";track_ids:string[]}|{kind:"playlist";playlist_id:string}|{kind:"folder";root_id:string;recursive:boolean}|{kind:"all_ready"};
export interface OrganizationRequest {scope:OrganizationScope;settings:OrganizationSettings}
export interface OrganizationTotals {total:number;ready:number;unchanged:number;skipped:number;blocked:number}
export interface OrganizationPreview {id:string;revision:string;created:number;totals:OrganizationTotals;can_apply:boolean;estimated_bytes?:number|null;available_bytes?:number|null}
export interface OrganizationRow {id:string;track_id:string;title:string;before_path:string;after_path:string|null;outcome:string;reason?:string|null;operation_id?:string|null;references:{id:string;target:string;label:string;state:string;reason?:string|null}[]}
export interface OrganizationPage {rows:OrganizationRow[];total:number;offset:number;limit:number}
export interface OrganizationBatch {id:string;preview_id?:string|null;created:number;kind:"apply"|"undo";state:"applying"|"complete"|"partial"|"cancelled"|"recovery_required"|"undone";totals:{total:number;completed:number;unchanged:number;skipped:number;blocked:number;failed:number;recovery_required:number;cancelled:number};can_undo:boolean;can_recover:boolean;resume_job_id?:string|null;error?:string|null}
export interface OrganizationExample {valid:boolean;error:string|null;tokens:string[];examples:{label:string;original_name:string;result:string|null;outcome:string;reason?:string|null}[]}
export interface OrganizationAlternate {operation_id:string;track_id:string;display_path:string;state:string;captured_sha256:string;observed_sha256?:string|null;ignored_sha256?:string|null}
export interface OrganizationBridge {
  organizationRoots():Promise<OrganizationRoot[]>;
  chooseOrganizationRoot():Promise<OrganizationRoot|null>;
  organizationExample(settings:OrganizationSettings):Promise<OrganizationExample>;
  previewOrganization(operationId:string,request:OrganizationRequest):Promise<OrganizationPreview>;
  organizationPreviewPage(id:string,offset:number,limit:number):Promise<OrganizationPage>;
  discardOrganizationPreview(id:string):Promise<void>;
  applyOrganization(operationId:string,previewId:string,revision:string):Promise<OrganizationBatch>;
  cancelOrganization(operationId:string):Promise<boolean>;
  cancelOrganizationBatch(id:string):Promise<void>;
  organizationHistory():Promise<OrganizationBatch[]>;
  organizationBatchPage(id:string,offset:number,limit:number):Promise<OrganizationPage>;
  recoverOrganization(operationId:string,id:string):Promise<OrganizationBatch>;
  undoOrganization(operationId:string,id:string):Promise<OrganizationBatch>;
  organizationAlternates():Promise<OrganizationAlternate[]>;
  resolveOrganizationAlternate(id:string,resolution:"ignore_this_revision"|"add_as_new_incoming",operationId:string):Promise<void>;
}
