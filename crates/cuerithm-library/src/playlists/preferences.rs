use super::*;
pub(crate) fn stamp(conn: &Connection) -> Result<SmartStamp> {
    Ok(SmartStamp {
        input_revision: conn.query_row(
            "SELECT value FROM meta WHERE key='smart_input_revision'",
            [],
            |r| r.get(0),
        )?,
        playlist_revision: revision(conn)?,
        preference_revision: conn.query_row(
            "SELECT value FROM meta WHERE key='playlist_preferences_revision'",
            [],
            |r| r.get(0),
        )?,
    })
}
pub(crate) fn preferences(conn: &Connection) -> Result<PlaylistPreferencesSnapshot> {
    let text: String = conn.query_row(
        "SELECT json FROM library_preferences WHERE singleton=1",
        [],
        |r| r.get(0),
    )?;
    Ok(PlaylistPreferencesSnapshot {
        revision: stamp(conn)?.preference_revision,
        preferences: serde_json::from_str(&text)?,
    })
}
pub(crate) fn zone(preferences: &PlaylistPreferences) -> Result<String> {
    let zone = match &preferences.time_zone {
        Some(t) => t.clone(),
        None => iana_time_zone::get_timezone()
            .context("Cannot detect system time zone; choose one in library preferences")?,
    };
    let _: chrono_tz::Tz = zone.parse()?;
    Ok(zone)
}
impl Library {
    pub fn resolve_playlist_calendar_day(
        &self,
        day: &str,
        expected_preferences: u64,
    ) -> Result<i64> {
        let conn = self.conn()?;
        let snapshot = preferences(&conn)?;
        ensure!(
            snapshot.revision == expected_preferences,
            "Library time zone changed; reload the editor"
        );
        cuerithm_smartlists::calendar_day_epoch(day, &zone(&snapshot.preferences)?)
    }
    pub fn get_playlist_preferences(&self) -> Result<PlaylistPreferencesSnapshot> {
        preferences(&*self.conn()?)
    }
    pub fn save_playlist_preferences(
        &self,
        value: PlaylistPreferences,
        expected: u64,
    ) -> Result<PlaylistPreferencesSnapshot> {
        if let Some(zone) = &value.time_zone {
            ensure!(zone.len() <= 128, "Invalid time zone");
            let _: chrono_tz::Tz = zone.parse()?;
        }
        let mut conn = self.conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let old = preferences(&tx)?;
        ensure!(
            old.revision == expected,
            "Library preferences changed; refresh before saving"
        );
        if old.preferences != value {
            tx.execute(
                "UPDATE library_preferences SET json=?1 WHERE singleton=1",
                [serde_json::to_string(&value)?],
            )?;
            tx.execute(
                "UPDATE meta SET value=value+1 WHERE key='playlist_preferences_revision'",
                [],
            )?;
            tx.execute(
                "UPDATE smart_playlist_status SET state='stale' WHERE state='ready'",
                [],
            )?;
            Library::bump(&tx)?;
        }
        let result = preferences(&tx)?;
        tx.commit()?;
        Ok(result)
    }
}
