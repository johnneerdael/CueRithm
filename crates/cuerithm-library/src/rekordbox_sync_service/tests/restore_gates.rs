use super::*;
use std::{
    sync::{Mutex, mpsc},
    thread,
    time::Duration,
};

#[derive(Clone, Copy)]
enum Gate {
    Organization,
    #[cfg(feature = "autotag")]
    Tag,
}
impl Gate {
    fn mutex(self, library: &Library) -> &Mutex<()> {
        match self {
            Self::Organization => &library.inner.organization_gate,
            #[cfg(feature = "autotag")]
            Self::Tag => &library.inner.tag_gate,
        }
    }
}
fn applied_fixture(recovery: bool) -> Fixture {
    let f = fixture(true);
    publication::fixture_apply(
        &f.library,
        &f.id,
        &key(),
        &CancellationToken::default(),
        &|| Ok(()),
    )
    .unwrap();
    if recovery {
        let mut op = load(&f.library, &f.id).unwrap();
        op.status = "restoring".into();
        save(&f.library.conn().unwrap(), &op).unwrap();
    }
    f
}
fn restore(
    f: &Fixture,
    recovery: bool,
    guard: &impl Fn() -> Result<()>,
) -> Result<RekordboxSyncOutcome> {
    if recovery {
        publication::fixture_recover(
            &f.library,
            &f.id,
            &key(),
            &CancellationToken::default(),
            guard,
        )
    } else {
        publication::fixture_restore(
            &f.library,
            &f.id,
            &key(),
            &CancellationToken::default(),
            guard,
        )
    }
}

// A real competing worker changes an original resource while holding the same
// publication gate. Restore must wait, then reject the newly changed proof
// without acquiring the native lease early or starting target mutation.
fn waits_for_worker_and_rechecks_proofs(gate: Gate) {
    for recovery in [false, true] {
        let f = applied_fixture(recovery);
        let op = load(&f.library, &f.id).unwrap();
        let ledger_after =
            digest(&ledger::capture(&f.library.conn().unwrap(), &op.native_target_id).unwrap())
                .unwrap();
        let held = gate.mutex(&f.library).lock().unwrap();
        let (started_tx, started_rx) = mpsc::channel();
        let (finished_tx, finished_rx) = mpsc::channel();
        let (guard_tx, guard_rx) = mpsc::channel();
        thread::scope(|scope| {
            let worker = scope.spawn(|| {
                started_tx.send(()).unwrap();
                let result = restore(&f, recovery, &|| {
                    guard_tx.send(())?;
                    Ok(())
                });
                finished_tx.send(result).unwrap();
            });
            started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            let early_guard = guard_rx.recv_timeout(Duration::from_millis(300));
            // The gated worker may move/write files, so prove those changes are
            // checked after waiting instead of trusting an earlier hash.
            fs::write(&f.old_asset, b"resource changed by the gated worker").unwrap();
            let early_native_lease =
                NativeDatabaseLease::acquire(&f.native, &key(), &CancellationToken::default());
            let lease_was_available = early_native_lease.is_ok();
            drop(early_native_lease);
            drop(held);
            let result = finished_rx.recv_timeout(Duration::from_secs(10)).unwrap();
            worker.join().unwrap();
            assert!(
                matches!(early_guard, Err(mpsc::RecvTimeoutError::Timeout)),
                "restore entered its protected phase while another worker held the gate"
            );
            assert!(
                lease_was_available,
                "restore acquired the native lease before waiting for the publication gate"
            );
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("Original native assets changed")
            );
        });
        assert_eq!(native_hash(&f), op.after.logical_sha256);
        assert!(f.new_asset.exists());
        assert_eq!(
            fs::read(&f.old_asset).unwrap(),
            b"resource changed by the gated worker"
        );
        assert_eq!(
            digest(&ledger::capture(&f.library.conn().unwrap(), &op.native_target_id).unwrap())
                .unwrap(),
            ledger_after
        );
        assert_eq!(
            status(&f.library, &f.id).unwrap(),
            if recovery { "restoring" } else { "applied" }
        );
    }
}

// Start a competing file/organization worker at restore's first process guard.
// It must remain excluded throughout native restore, asset removal and the final
// ledger commit. Once admitted it must observe the entire restored state.
fn excludes_worker_until_restored_commit(gate: Gate) {
    for recovery in [false, true] {
        let f = applied_fixture(recovery);
        let op = load(&f.library, &f.id).unwrap();
        let (start_tx, start_rx) = mpsc::channel();
        let (attempt_tx, attempt_rx) = mpsc::channel();
        let (acquired_tx, acquired_rx) = mpsc::channel();
        let first = Cell::new(true);
        let saw_asset_phase = Cell::new(false);
        thread::scope(|scope| {
            let f = &f;
            let op = &op;
            let worker = scope.spawn(move || {
                start_rx.recv_timeout(Duration::from_secs(10)).unwrap();
                attempt_tx.send(()).unwrap();
                let _gate = gate.mutex(&f.library).lock().unwrap();
                acquired_tx.send(()).unwrap();
                // Use an independent reader: Library::conn would wait on the
                // restore transaction's mutex and hide premature gate release.
                assert_eq!(status(&f.library, &f.id).unwrap(), "restored");
                assert_eq!(native_hash(f), op.before.logical_sha256);
                assert!(!f.new_asset.exists());
                assert_eq!(
                    digest(
                        &ledger::capture(
                            &Connection::open(f.library.data_dir().join("library.db")).unwrap(),
                            &op.native_target_id,
                        )
                        .unwrap()
                    )
                    .unwrap(),
                    digest(&op.ledger_before).unwrap()
                );
            });
            let result = restore(f, recovery, &|| {
                if first.replace(false) {
                    start_tx.send(())?;
                    attempt_rx.recv_timeout(Duration::from_secs(5))?;
                    ensure!(
                        matches!(
                            acquired_rx.recv_timeout(Duration::from_millis(300)),
                            Err(mpsc::RecvTimeoutError::Timeout)
                        ),
                        "competing worker entered during protected restore"
                    );
                } else {
                    ensure!(
                        matches!(acquired_rx.try_recv(), Err(mpsc::TryRecvError::Empty)),
                        "publication gate released before restore committed"
                    );
                }
                if !f.new_asset.exists() {
                    // Include callbacks after asset removal; the worker must
                    // still wait for the committed local acknowledgement.
                    saw_asset_phase.set(true);
                }
                Ok(())
            });
            result.unwrap();
            acquired_rx.recv_timeout(Duration::from_secs(10)).unwrap();
            worker.join().unwrap();
        });
        assert!(
            saw_asset_phase.get(),
            "exercise the guard after asset restoration and before local commit"
        );
    }
}

#[test]
fn restore_and_restoring_recovery_wait_for_organization_worker_then_recheck_proofs() {
    waits_for_worker_and_rechecks_proofs(Gate::Organization);
}
#[test]
fn restore_and_restoring_recovery_exclude_organization_worker_through_ledger_commit() {
    excludes_worker_until_restored_commit(Gate::Organization);
}
#[cfg(feature = "autotag")]
#[test]
fn restore_and_restoring_recovery_wait_for_tag_worker_then_recheck_proofs() {
    waits_for_worker_and_rechecks_proofs(Gate::Tag);
}
#[cfg(feature = "autotag")]
#[test]
fn restore_and_restoring_recovery_exclude_tag_worker_through_ledger_commit() {
    excludes_worker_until_restored_commit(Gate::Tag);
}
