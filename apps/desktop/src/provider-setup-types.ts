export type CatalogProviderId = "beatport"|"traxsource"|"musicbrainz"|"sonovault"|"discogs"|"itunes"|"bpmsupreme"|"bandcamp"|"deezer"|"volumo";
export type CredentialProviderId = "sonovault"|"discogs"|"bpm_supreme"|"volumo";
export interface CredentialRef {provider:CredentialProviderId;generation:string;session:string}
export interface CredentialStatus {
 reference:CredentialRef;
 presence:"present"|"missing"|"unknown";
 vault:"ready"|"locked"|"unavailable"|"denied"|"corrupt"|"ambiguous"|"uncertain";
 verification:"untested"|"valid"|"invalid"|"email_unverified"|"restricted"|"rate_limited"|"quota_exhausted";
}
export type ProviderCredentialInput = {kind:"api_key"|"personal_token";secret:string}|{kind:"email_password";email:string;password:string};
export interface ProviderSetupStatus {
 id:CatalogProviderId;revision:string;
 setup_kind:"none"|"api_key"|"personal_token"|"email_password";
 credential:CredentialStatus|null;
 auth_mode:"public"|"anonymous"|"authenticated"|"unavailable";
 can_use:boolean;can_test?:boolean;
 reason:{code:string;message:string}|null;
 rate_limit_per_minute:number|null;min_request_interval_ms:number|null;
 retry_at_ms:number|null;
 quota:{remaining:number|null;limit:number|null;resets_at_ms:number|null}|null;
}
export interface ProviderSetupBridge {
 providerSetupStatuses():Promise<ProviderSetupStatus[]>;
 saveProviderCredential(expected:CredentialRef,input:ProviderCredentialInput):Promise<CredentialStatus>;
 testProviderCredential(operationId:string,expected:CredentialRef):Promise<ProviderSetupStatus>;
 removeProviderCredential(provider:CredentialProviderId):Promise<CredentialStatus>;
}
