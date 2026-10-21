import type {MetadataDate as TransferDate} from "./editor-types";
export interface CatalogArtistCredit {artist_id:string;name:string;credited_name:string;join_phrase:string}
export interface CatalogPosition {medium_position:number|null;track_position:number|null;printed_number:string|null;medium_track_count:number|null;medium_count:number|null}
export interface MusicBrainzFacts {
 schema_version:number;recording_id:string;release_id:string|null;release_group_id:string|null;release_track_id:string|null;scope:"recording_only"|"release_track";
 recording_title:string;track_title:string|null;recording_disambiguation:string|null;
 recording_credit:CatalogArtistCredit[];track_credit:CatalogArtistCredit[];release_credit:CatalogArtistCredit[];
 isrcs:string[];recording_length_ms:number|null;track_length_ms:number|null;
 selected_release_date:TransferDate|null;recording_first_release_date:TransferDate|null;release_group_first_release_date:TransferDate|null;
 release_events:{date:TransferDate|null;area_id:string|null;country:string|null}[];
 position:CatalogPosition;
 release:{title:string;status:string|null;country:string|null;barcode:string|null;media_format:string|null;labels:{id:string|null;name:string|null;catalog_number:string|null}[]}|null;
 cover:{release_id:string;image_id:string|null}|null;
 source_policy:{adapter_revision:string;policy_revision:string;data_class:string};
}
export interface SourceCatalogFacts {
 schema_version:number;provider:string;release_date:TransferDate|null;release_id:string|null;raw_position:string|null;
 identities:{source:string;kind:"recording"|"release"|"release_track"|"artist"|"platform_track";id:string}[];
 links:{source:string;url:string}[];
}
export type CatalogFacts={kind:"musicbrainz";data:MusicBrainzFacts}|{kind:"source";data:SourceCatalogFacts};
