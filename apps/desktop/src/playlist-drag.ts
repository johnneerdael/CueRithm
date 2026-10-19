import type {TrackQuery} from "./types";
export type PlaylistDragPayload={kind:"playlist";id:string;revision:number}|{kind:"tracks";ids:string[];query:TrackQuery;revision:number};
export type PlaylistDropTarget={kind:"playlist";id:string}|{kind:"track";id:string}|{kind:"root"};
