use super::*;

impl Library {
    pub fn preview_import(
        &self,
        request: ImportPreviewRequest,
        cancel: &CancellationToken,
    ) -> Result<ImportPreview> {
        cancel.check()?;
        ensure!(
            request.policy.version == 1,
            "Unsupported import policy version"
        );
        ensure!(
            serde_json::to_vec(&request)?.len() <= ImportLimits::default().text_bytes as usize,
            "Import selection exceeds request byte limit"
        );
        let mut decisions = std::collections::BTreeSet::new();
        for d in &request.decisions {
            ensure!(
                decisions.insert((&d.external_id, &d.field)),
                "Duplicate import conflict decision"
            );
        }
        let (snapshot, nonce) = self.guarded_import_snapshot(&request.inspection_id)?;
        snapshot.validate_source(cancel)?;
        let (source, header, profile, inspect, local_revision) = {
            let conn = self.conn()?;
            let (source,header,profile,inspect):(String,String,String,String)=conn.query_row("SELECT source_id,header_json,profile_json,request_json FROM import_snapshots WHERE id=?1",[&request.inspection_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
            (
                source,
                serde_json::from_str::<ObservationHeader>(&header)?,
                serde_json::from_str::<SourceProfile>(&profile)?,
                serde_json::from_str::<ImportInspectRequest>(&inspect)?,
                revision(&conn)?,
            )
        };
        ensure!(
            !request.policy.capture_file_tags || inspect.capture_file_tags,
            "File tags were not captured; inspect again with file-tag capture enabled"
        );
        ensure!(
            !request.policy.include_history || inspect.include_history,
            "History was not captured"
        );
        ensure!(
            !request.policy.include_queue || inspect.include_queue,
            "Queue was not captured"
        );
        let processing = super::super::processing::freeze(self, &request.policy, cancel)?;
        // Media hashing and path resolution run outside the library lock. Disk candidates
        // bound memory independently of the number of selected records.
        let spool_file = tempfile::NamedTempFile::new()?;
        let spool = Connection::open(spool_file.path())?;
        spool.execute_batch(
            "CREATE TABLE candidates(sequence INTEGER PRIMARY KEY,json TEXT NOT NULL)",
        )?;
        let mut cursor = None;
        let mut sequence = 0i64;
        loop {
            let page = self.import_page(
                &request.inspection_id,
                ImportPageQuery {
                    kind: ImportRecordKind::Track,
                    limit: 100,
                    cursor,
                },
            )?;
            for record in page.records {
                cancel.check()?;
                let ObservationRecord::Track(track) = record else {
                    unreachable!()
                };
                if !request.tracks.contains(&track.external_id) {
                    continue;
                }
                let location = resolve_location(&track.locations, &inspect.path_mappings)?;
                let automatic = location
                    .resolved
                    .as_deref()
                    .map(|path| {
                        identity::exact_path(self, path, &track, location.mapping_root_id.is_some())
                    })
                    .transpose()?
                    .flatten();
                let (item_id, local_track_id, is_new, current_path, established) = {
                    let conn = self.conn()?;
                    let binding:Option<String>=conn.query_row("SELECT item_id FROM import_bindings WHERE source_id=?1 AND external_id=?2",params![source,track.external_id],|r|r.get(0)).optional()?;
                    let target = request.target_items.get(&track.external_id);
                    ensure!(
                        binding
                            .as_ref()
                            .is_none_or(|id| target.is_none_or(|t| id == t)),
                        "Established source binding cannot be reassigned"
                    );
                    let established = binding.is_some();
                    let item_id = binding
                        .or_else(|| target.cloned())
                        .or(automatic)
                        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                    let local_path: Option<Vec<u8>> = conn
                        .query_row("SELECT path FROM tracks WHERE id=?1", [&item_id], |r| {
                            r.get(0)
                        })
                        .optional()?;
                    let imported: bool = conn.query_row(
                        "SELECT EXISTS(SELECT 1 FROM import_items WHERE id=?1)",
                        [&item_id],
                        |r| r.get(0),
                    )?;
                    ensure!(
                        target.is_none() || local_path.is_some() || imported,
                        "Unknown reviewed identity target"
                    );
                    let local = local_path.as_ref().map(|_| item_id.clone());
                    let is_new = local.is_none() && !imported;
                    let path = local_path.map(|p| bytes_path(&p)).transpose()?;
                    (item_id, local, is_new, path, established)
                };
                let resolved = current_path.or(location.resolved);
                let media = resolved
                    .as_deref()
                    .map(|p| MediaGuard::capture(p, cancel))
                    .transpose()?;
                let availability = media
                    .as_ref()
                    .map(|m| {
                        if m.sha256.is_some() {
                            "available"
                        } else {
                            "missing"
                        }
                    })
                    .unwrap_or(&track.locations.availability)
                    .to_owned();
                let mut identity_change = identity_change(&*self.conn()?, &source, &track)?;
                if !established
                    && is_new
                    && request.policy.allow_content_matching
                    && let Some(media) = &media
                {
                    let candidates = identity::content_candidates(self, media, cancel)?;
                    if !candidates.is_empty() {
                        identity_change = Some(
                            json!({"reason":"review_content_candidates","candidates":candidates}),
                        );
                    }
                }
                let identity_blocked = identity_change.is_some()
                    && !matches!(
                        decision(&request, &track.external_id, "identity"),
                        Some(ImportDecision::UseSource)
                    );
                let candidate = Candidate {
                    external_id: track.external_id.clone(),
                    item_id,
                    local_track_id,
                    is_new,
                    media,
                    availability,
                    identity_change,
                    identity_blocked,
                };
                spool.execute(
                    "INSERT INTO candidates VALUES(?1,?2)",
                    params![sequence, serde_json::to_string(&candidate)?],
                )?;
                sequence += 1;
            }
            cursor = page.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        snapshot.validate_source(cancel)?;
        let preview_id = uuid::Uuid::new_v4().to_string();
        let policy_id = uuid::Uuid::new_v4().to_string();
        let mut summary = ImportPreview {
            preview_id: preview_id.clone(),
            processing_requested: false,
            ..Default::default()
        };
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure!(
            revision(&tx)? == local_revision,
            "Library changed during import preview"
        );
        tx.execute(
            "INSERT INTO import_policies VALUES(?1,?2,?3,?4)",
            params![
                policy_id,
                source,
                serde_json::to_string(&request.policy)?,
                now()
            ],
        )?;
        tx.execute("INSERT INTO import_previews(id,source_id,snapshot_id,local_revision,policy_id,json,runtime_nonce,created) VALUES(?1,?2,?3,?4,?5,'{}',?6,?7)",params![preview_id,source,request.inspection_id,local_revision,policy_id,nonce,now()])?;
        let mut prepared = spool.prepare("SELECT json FROM candidates ORDER BY sequence")?;
        let mut candidates = prepared.query([])?;
        let mut effect_sequence = 0i64;
        while let Some(row) = candidates.next()? {
            cancel.check()?;
            let candidate: Candidate = serde_json::from_str(&row.get::<_, String>(0)?)?;
            let track = load_track(&tx, &request.inspection_id, &candidate.external_id)?;
            let local = local_fields(&tx, &candidate.item_id)?;
            let incoming = selected_source_values(&tx, &track, &request.policy)?;
            let mut fields = BTreeMap::new();
            let mut baselines = BTreeMap::new();
            let mut provenance = BTreeMap::new();
            let mut effects = Vec::new();
            for (field, value) in incoming {
                let before = local.get(&field);
                let applied = baseline(&tx, &source, &candidate.external_id, &field)?;
                let choice = decision(&request, &candidate.external_id, &field);
                let (mut adopted, conflict) =
                    choose_field(&value, before, applied.as_ref(), choice);
                let mut blocked_reason = None;
                if !readable(&profile, &field)? {
                    adopted = None;
                    blocked_reason = Some(
                        "This exact source profile has no qualified read capability for this field"
                            .into(),
                    );
                }
                if field == "cues.hot"
                    && occupied(before)
                    && adopted.as_ref().is_some_and(|v| Some(v) != before)
                    && (!request.policy.replace_hotcues
                        || !matches!(
                            choice,
                            Some(ImportDecision::UseSource | ImportDecision::Custom(_))
                        ))
                {
                    adopted = None;
                    blocked_reason=Some("Replacing occupied hotcues requires an explicit source/custom decision and hotcue replacement permission".into());
                }
                if value.get("presence").and_then(Value::as_str) == Some("deleted")
                    && (!matches!(header.completeness, Completeness::WholeLibrary)
                        || !matches!(choice, Some(ImportDecision::UseSource)))
                {
                    adopted = None;
                    blocked_reason=Some("Deletion requires a complete authoritative observation and an explicit decision".into());
                }
                if candidate.identity_blocked {
                    adopted = None;
                    blocked_reason=Some("Source identity/location changed; review possible reused ID before adopting this record".into());
                }
                if let Some(accepted) = &adopted {
                    validate_custom_field(&field, accepted)?;
                    if before != Some(accepted) {
                        fields.insert(field.clone(), accepted.clone());
                        summary.fields_changed += 1;
                    }
                    if applied.as_ref() != Some(&(value.clone(), accepted.clone())) {
                        baselines.insert(field.clone(), (value.clone(), accepted.clone()));
                        summary.baselines_changed += 1;
                    }
                    let kind = if matches!(choice, Some(ImportDecision::Custom(_))) {
                        "custom"
                    } else if value.get("provenance").and_then(Value::as_str) == Some("file_tags")
                        || (field.starts_with("metadata.")
                            && track
                                .metadata
                                .get(field.trim_start_matches("metadata."))
                                .is_none_or(|v| matches!(v, Presence::Absent))
                            && request.policy.capture_file_tags)
                    {
                        "file_tags"
                    } else {
                        "source"
                    };
                    provenance.insert(field.clone(),json!({"kind":kind,"source_id":source,"snapshot_id":request.inspection_id,"external_id":candidate.external_id}));
                }
                summary.conflicts += u64::from(conflict);
                summary.blocked += u64::from(blocked_reason.is_some());
                effects.push(ImportFieldEffect {
                    field,
                    source: Some(value),
                    local: before.cloned(),
                    proposed: adopted,
                    conflict,
                    blocked_reason,
                });
            }
            if let Some(previous) = &candidate.identity_change {
                summary.conflicts += u64::from(candidate.identity_blocked);
                effects.push(ImportFieldEffect{field:"identity".into(),source:Some(serde_json::to_value(&track.locations)?),local:Some(previous.clone()),proposed:if candidate.identity_blocked{None}else{Some(serde_json::to_value(&track.locations)?)},conflict:candidate.identity_blocked,blocked_reason:if candidate.identity_blocked{Some("Confirm source identity before adopting a changed location or reused ID".into())}else{None}});
            }
            summary.tracks_created += u64::from(candidate.is_new && !candidate.identity_blocked);
            let effect = ImportPreviewEffect {
                external_id: candidate.external_id.clone(),
                item_id: candidate.item_id.clone(),
                kind: "track".into(),
                fields: effects,
            };
            let mut projected = local;
            projected.extend(fields.clone());
            let process = super::super::processing::has_stages(&request.policy, &projected)
                && processing_required(
                    &tx,
                    &source,
                    &request.policy,
                    &processing,
                    &candidate,
                    !fields.is_empty(),
                )?;
            summary.processing_requested |= process;
            let proposal = TrackProposal {
                candidate,
                fields,
                baselines,
                provenance,
                process,
            };
            tx.execute(
                "INSERT INTO import_preview_effects VALUES(?1,?2,'track',?3,?4,?5)",
                params![
                    preview_id,
                    effect_sequence,
                    effect.external_id,
                    serde_json::to_string(&proposal)?,
                    serde_json::to_string(&effect)?
                ],
            )?;
            effect_sequence += 1;
        }
        collections::stage_collections(
            &tx,
            collections::Context {
                request: &request,
                source: &source,
                header: &header,
                profile: &profile,
                preview_id: &preview_id,
            },
            &mut summary,
            &mut effect_sequence,
            cancel,
        )?;
        tx.execute(
            "UPDATE import_previews SET json=?1 WHERE id=?2",
            params![
                serde_json::to_string(&PreviewMeta {
                    request,
                    summary: summary.clone(),
                    processing,
                })?,
                preview_id
            ],
        )?;
        cancel.check()?;
        tx.commit()?;
        Ok(summary)
    }
}
