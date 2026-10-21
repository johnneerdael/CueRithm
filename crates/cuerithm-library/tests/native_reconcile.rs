use cuerithm_library::{
    NativeConflict, NativeFieldBaseline, NativeFieldDecision, NativeFieldObservation,
    NativeFieldOwner, NativeReconciliation, reconcile_field,
};
use serde_json::{Value, json};

fn baseline(owner: NativeFieldOwner, acknowledged: bool) -> NativeFieldBaseline {
    NativeFieldBaseline {
        local: json!("before"),
        remote: json!("before"),
        owner,
        acknowledged,
    }
}

fn observed(value: Value) -> NativeFieldObservation {
    NativeFieldObservation::Present(value)
}

#[test]
fn initial_equal_values_still_require_explicit_acknowledgement() {
    assert_eq!(
        reconcile_field(
            &baseline(NativeFieldOwner::Shared, false),
            &observed(json!("before")),
            &observed(json!("before")),
        ),
        NativeReconciliation::Conflict {
            reason: NativeConflict::InitialAcknowledgement,
            suggestion: Some(NativeFieldDecision::AcceptEqual),
        }
    );
}

#[test]
fn missing_observation_is_distinct_from_an_observed_null_or_clear_decision() {
    let before = NativeFieldBaseline {
        local: Value::Null,
        remote: Value::Null,
        owner: NativeFieldOwner::Shared,
        acknowledged: true,
    };
    assert_eq!(
        reconcile_field(
            &before,
            &NativeFieldObservation::Unavailable,
            &observed(Value::Null),
        ),
        NativeReconciliation::Conflict {
            reason: NativeConflict::ObservationUnavailable,
            suggestion: None,
        }
    );
    assert_eq!(
        reconcile_field(&before, &observed(Value::Null), &observed(Value::Null)),
        NativeReconciliation::Unchanged
    );
    assert_ne!(
        serde_json::to_value(NativeFieldDecision::ClearRemote).unwrap(),
        serde_json::to_value(NativeFieldDecision::UseLocal).unwrap()
    );
}

#[test]
fn shared_one_sided_changes_propose_the_changed_value_without_mutating_baselines() {
    let before = baseline(NativeFieldOwner::Shared, true);
    assert_eq!(
        reconcile_field(&before, &observed(json!("new")), &observed(json!("before"))),
        NativeReconciliation::Proposed {
            decision: NativeFieldDecision::UseLocal,
        }
    );
    assert_eq!(
        reconcile_field(&before, &observed(json!("before")), &observed(json!("new"))),
        NativeReconciliation::Proposed {
            decision: NativeFieldDecision::UseRemote,
        }
    );
    assert_eq!(before.local, json!("before"));
    assert_eq!(before.remote, json!("before"));
}

#[test]
fn divergent_concurrent_changes_always_need_review_even_with_an_owner() {
    for owner in [
        NativeFieldOwner::Shared,
        NativeFieldOwner::Local,
        NativeFieldOwner::Remote,
        NativeFieldOwner::Unowned,
    ] {
        let result = reconcile_field(
            &baseline(owner, true),
            &observed(json!("local edit")),
            &observed(json!("remote edit")),
        );
        assert!(matches!(
            result,
            NativeReconciliation::Conflict {
                reason: NativeConflict::ConcurrentChanges,
                ..
            }
        ));
    }
}

#[test]
fn an_edit_against_the_other_owner_is_not_silently_overwritten() {
    for (owner, local, remote, suggestion) in [
        (
            NativeFieldOwner::Local,
            "before",
            "new",
            NativeFieldDecision::UseLocal,
        ),
        (
            NativeFieldOwner::Remote,
            "new",
            "before",
            NativeFieldDecision::UseRemote,
        ),
    ] {
        assert_eq!(
            reconcile_field(
                &baseline(owner, true),
                &observed(json!(local)),
                &observed(json!(remote)),
            ),
            NativeReconciliation::Conflict {
                reason: NativeConflict::Ownership,
                suggestion: Some(suggestion),
            }
        );
    }
}

#[test]
fn unowned_changed_fields_are_never_automatically_adopted() {
    assert_eq!(
        reconcile_field(
            &baseline(NativeFieldOwner::Unowned, true),
            &observed(json!("new")),
            &observed(json!("before")),
        ),
        NativeReconciliation::Conflict {
            reason: NativeConflict::Unowned,
            suggestion: None,
        }
    );
}

#[test]
fn converging_changes_propose_acknowledgement_without_claiming_publication() {
    assert_eq!(
        reconcile_field(
            &baseline(NativeFieldOwner::Shared, true),
            &observed(json!("same edit")),
            &observed(json!("same edit")),
        ),
        NativeReconciliation::Proposed {
            decision: NativeFieldDecision::AcceptEqual,
        }
    );
}

#[test]
fn normalized_local_and_remote_baselines_can_differ_without_being_changes() {
    let before = NativeFieldBaseline {
        local: json!("8A"),
        remote: json!("Am"),
        owner: NativeFieldOwner::Shared,
        acknowledged: true,
    };
    assert_eq!(
        reconcile_field(&before, &observed(json!("8A")), &observed(json!("Am"))),
        NativeReconciliation::Unchanged
    );
}

#[test]
fn arrays_and_explicit_nulls_are_compared_losslessly() {
    let before = NativeFieldBaseline {
        local: json!(["Artist", "Guest"]),
        remote: json!(["Artist", "Guest"]),
        owner: NativeFieldOwner::Shared,
        acknowledged: true,
    };
    assert_eq!(
        reconcile_field(
            &before,
            &observed(json!([])),
            &observed(before.remote.clone())
        ),
        NativeReconciliation::Proposed {
            decision: NativeFieldDecision::UseLocal,
        }
    );
    assert_eq!(
        reconcile_field(
            &before,
            &observed(Value::Null),
            &observed(before.remote.clone())
        ),
        NativeReconciliation::Proposed {
            decision: NativeFieldDecision::UseLocal,
        }
    );
}
