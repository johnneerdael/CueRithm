//! Side-effect-free reconciliation proposals. Observations and proposals never
//! acknowledge synchronization or authorize native/file publication.
use crate::{NativeFieldBaseline, NativeFieldOwner};
use serde::{Deserialize, Serialize};
use serde_json::Value;
mod review;
pub use review::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", content = "value", rename_all = "snake_case")]
pub enum NativeFieldObservation {
    Unavailable,
    Present(Value),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeFieldDecision {
    UseLocal,
    UseRemote,
    ClearLocal,
    ClearRemote,
    AcceptEqual,
    Skip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeConflict {
    ObservationUnavailable,
    InitialAcknowledgement,
    ConcurrentChanges,
    Ownership,
    Unowned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum NativeReconciliation {
    Unchanged,
    Proposed {
        decision: NativeFieldDecision,
    },
    Conflict {
        reason: NativeConflict,
        suggestion: Option<NativeFieldDecision>,
    },
}

/// Compare each side with its own acknowledged representation. In particular,
/// an acknowledged Camelot key and musical key need not have identical JSON.
/// A proposal still requires reviewed, guarded publication by the coordinator.
pub fn reconcile_field(
    baseline: &NativeFieldBaseline,
    local: &NativeFieldObservation,
    remote: &NativeFieldObservation,
) -> NativeReconciliation {
    use NativeFieldDecision::{AcceptEqual, UseLocal, UseRemote};
    use NativeReconciliation::{Conflict, Proposed, Unchanged};
    let (NativeFieldObservation::Present(local), NativeFieldObservation::Present(remote)) =
        (local, remote)
    else {
        return Conflict {
            reason: NativeConflict::ObservationUnavailable,
            suggestion: None,
        };
    };
    let owner_suggestion = match baseline.owner {
        NativeFieldOwner::Local => Some(UseLocal),
        NativeFieldOwner::Remote => Some(UseRemote),
        NativeFieldOwner::Shared | NativeFieldOwner::Unowned => None,
    };
    if !baseline.acknowledged {
        return Conflict {
            reason: NativeConflict::InitialAcknowledgement,
            suggestion: if local == remote {
                Some(AcceptEqual)
            } else {
                owner_suggestion
            },
        };
    }
    let local_changed = local != &baseline.local;
    let remote_changed = remote != &baseline.remote;
    if !local_changed && !remote_changed {
        return Unchanged;
    }
    if local_changed && remote_changed && local != remote {
        return Conflict {
            reason: NativeConflict::ConcurrentChanges,
            suggestion: owner_suggestion,
        };
    }
    if baseline.owner == NativeFieldOwner::Unowned {
        return Conflict {
            reason: NativeConflict::Unowned,
            suggestion: None,
        };
    }
    if local == remote {
        return Proposed {
            decision: AcceptEqual,
        };
    }
    if (local_changed && baseline.owner == NativeFieldOwner::Remote)
        || (remote_changed && baseline.owner == NativeFieldOwner::Local)
    {
        return Conflict {
            reason: NativeConflict::Ownership,
            suggestion: owner_suggestion,
        };
    }
    Proposed {
        decision: if local_changed { UseLocal } else { UseRemote },
    }
}
