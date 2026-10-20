import {invoke} from "@tauri-apps/api/core";
import {listen} from "@tauri-apps/api/event";
import type {MatchReviewBridge} from "./match-review-types";
export const nativeMatchReview:MatchReviewBridge={
 trackMatchReview:trackId=>invoke("track_match_review",{trackId}),
 previewMatchReview:(operationId,request)=>invoke("preview_match_review",{operationId,request}),
 applyMatchReview:(operationId,id,revision)=>invoke("apply_match_review",{operationId,previewId:id,revision}),
 requeryMatches:(operationId,trackId,generation)=>invoke("requery_matches",{operationId,trackId,generation}),
 matchReviewArtwork:(operationId,previewId,revision)=>invoke("match_review_artwork",{operationId,previewId,revision}),
 onMatchReviewChange:handler=>listen("match-review-change",()=>handler()),
};
