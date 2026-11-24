//! The Library's instance groups, in app.db. An instance's groups are composed onto it in the
//! frontend, beside the engine's instance.

use std::collections::{HashMap, HashSet};

use anyhow::bail;
use engine::ErrorKind;
use engine::data::EditInstance;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::local_api::events::AppEvent;
use crate::state::Desktop;

const MAX_GROUP_NAME_LENGTH: usize = 256;
pub const FAVORITES_GROUP_ID: &str = "group:favorites";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "export-ts", ts(rename = "InstanceGroupDefinition"))]
pub struct InstanceGroup {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct InstanceGroupMembershipUpdate {
    pub instance_id: String,
    pub group_ids: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct InstanceGroupsChangedPayload {
    pub instance_ids: Vec<String>,
}

fn validate_group_name(name: &str) -> engine::Result<&str> {
    let name = name.trim();

    if name.is_empty() {
        bail!(ErrorKind::InputError(
            "group name cannot be empty".to_string(),
        ));
    }

    if name.chars().count() > MAX_GROUP_NAME_LENGTH {
        bail!(ErrorKind::InputError(format!(
            "group name cannot exceed {MAX_GROUP_NAME_LENGTH} characters"
        )));
    }

    if name.eq_ignore_ascii_case("none") {
        bail!(ErrorKind::InputError(
            "group name cannot be None".to_string(),
        ));
    }

    Ok(name)
}

fn normalize_membership_updates(
    mut updates: Vec<InstanceGroupMembershipUpdate>,
) -> engine::Result<Vec<InstanceGroupMembershipUpdate>> {
    let mut instance_ids = HashSet::new();
    for update in &mut updates {
        if !instance_ids.insert(update.instance_id.clone()) {
            bail!(ErrorKind::InputError(format!(
                "duplicate instance {} in group membership update",
                update.instance_id
            )));
        }

        let mut group_ids = HashSet::new();
        update
            .group_ids
            .retain(|group_id| group_ids.insert(group_id.clone()));
    }

    Ok(updates)
}

fn changed(state: &Desktop, instance_ids: Vec<String>) {
    state.emit(AppEvent::InstanceGroupsChanged(
        InstanceGroupsChangedPayload { instance_ids },
    ));
}

pub async fn list_groups(
    db: &SqlitePool,
) -> engine::Result<Vec<InstanceGroup>> {
    Ok(sqlx::query_as!(
        InstanceGroup,
        "
		SELECT id, name
		FROM instance_groups
		ORDER BY display_order, name, id
		",
    )
    .fetch_all(db)
    .await?)
}

/// Each instance's groups, by instance id, ordered by group name.
pub async fn list_memberships(
    db: &SqlitePool,
) -> engine::Result<HashMap<String, Vec<String>>> {
    let rows = sqlx::query!(
        "
		SELECT memberships.instance_id, memberships.group_id
		FROM instance_group_memberships memberships
		INNER JOIN instance_groups groups
			ON groups.id = memberships.group_id
		ORDER BY groups.name
		",
    )
    .fetch_all(db)
    .await?;
    let mut memberships = HashMap::<String, Vec<String>>::new();
    for row in rows {
        memberships
            .entry(row.instance_id)
            .or_default()
            .push(row.group_id);
    }
    Ok(memberships)
}

pub async fn create_group(
    state: &Desktop,
    name: String,
) -> engine::Result<InstanceGroup> {
    let name = validate_group_name(&name)?;
    let id = Uuid::new_v4().to_string();
    let mut tx = state.db().begin_with("BEGIN IMMEDIATE").await?;
    sqlx::query!(
        "
		UPDATE instance_groups
		SET display_order = display_order + 1
		WHERE id != ?
		",
        FAVORITES_GROUP_ID,
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "
		INSERT INTO instance_groups (id, name, display_order)
		VALUES (?, ?, 0)
		",
        id,
        name,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(InstanceGroup {
        id,
        name: name.to_string(),
    })
}

/// Orders the groups as `group_ids` lists them, the unlisted ones after.
pub async fn set_group_order(
    state: &Desktop,
    group_ids: Vec<String>,
) -> engine::Result<()> {
    let mut tx = state.db().begin_with("BEGIN IMMEDIATE").await?;
    let existing_group_ids = sqlx::query_scalar!(
        "
		SELECT id
		FROM instance_groups
		WHERE id != ?
		ORDER BY display_order, name, id
		",
        FAVORITES_GROUP_ID,
    )
    .fetch_all(&mut *tx)
    .await?;
    let existing_group_id_set = existing_group_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut seen_group_ids = HashSet::new();
    let mut ordered_group_ids = Vec::with_capacity(existing_group_ids.len());

    for group_id in &group_ids {
        if !seen_group_ids.insert(group_id.as_str()) {
            bail!(ErrorKind::InputError(format!(
                "duplicate instance group {group_id} in group order"
            )));
        }

        if !existing_group_id_set.contains(group_id.as_str()) {
            bail!(ErrorKind::InputError(format!(
                "unknown instance group {group_id}"
            )));
        }

        ordered_group_ids.push(group_id.as_str());
    }

    for group_id in &existing_group_ids {
        if seen_group_ids.insert(group_id.as_str()) {
            ordered_group_ids.push(group_id.as_str());
        }
    }

    for (display_order, group_id) in ordered_group_ids.into_iter().enumerate() {
        let display_order = display_order as i64;
        sqlx::query!(
            "
			UPDATE instance_groups
			SET display_order = ?
			WHERE id = ?
			",
            display_order,
            group_id,
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(())
}

/// Replaces the groups of each instance named. A changed instance counts as modified.
pub async fn set_group_memberships(
    state: &Desktop,
    updates: Vec<InstanceGroupMembershipUpdate>,
) -> engine::Result<()> {
    if updates.is_empty() {
        return Ok(());
    }

    let updates = normalize_membership_updates(updates)?;
    for update in &updates {
        if engine::instance::get(state, &update.instance_id)
            .await?
            .is_none()
        {
            bail!(ErrorKind::InputError(format!(
                "unknown instance {}",
                update.instance_id
            )));
        }
    }

    let mut tx = state.db().begin_with("BEGIN IMMEDIATE").await?;
    let unique_group_ids = updates
        .iter()
        .flat_map(|update| update.group_ids.iter())
        .collect::<HashSet<_>>();

    for group_id in unique_group_ids {
        let exists = sqlx::query_scalar!(
            r#"SELECT EXISTS(SELECT 1 FROM instance_groups WHERE id = ?) AS "exists!: bool""#,
            group_id,
        )
        .fetch_one(&mut *tx)
        .await?;
        if !exists {
            bail!(ErrorKind::InputError(format!(
                "unknown instance group {group_id}"
            )));
        }
    }

    for update in &updates {
        sqlx::query!(
            "DELETE FROM instance_group_memberships WHERE instance_id = ?",
            update.instance_id,
        )
        .execute(&mut *tx)
        .await?;
        for group_id in &update.group_ids {
            sqlx::query!(
                "
				INSERT OR IGNORE INTO instance_group_memberships (instance_id, group_id)
				VALUES (?, ?)
				",
                update.instance_id,
                group_id,
            )
            .execute(&mut *tx)
            .await?;
        }
    }

    tx.commit().await?;

    for update in &updates {
        engine::instance::edit(
            state,
            &update.instance_id,
            EditInstance::default(),
        )
        .await?;
    }
    changed(
        state,
        updates
            .into_iter()
            .map(|update| update.instance_id)
            .collect(),
    );

    Ok(())
}

pub async fn rename_group(
    state: &Desktop,
    id: String,
    new_name: String,
) -> engine::Result<InstanceGroup> {
    if id == FAVORITES_GROUP_ID {
        bail!(ErrorKind::InputError(
            "Favorites cannot be renamed".to_string(),
        ));
    }

    let new_name = validate_group_name(&new_name)?;
    let mut tx = state.db().begin_with("BEGIN IMMEDIATE").await?;
    let instance_ids = members(&id, &mut tx).await?;
    let result = sqlx::query!(
        "UPDATE instance_groups SET name = ? WHERE id = ?",
        new_name,
        id,
    )
    .execute(&mut *tx)
    .await?;
    if result.rows_affected() == 0 {
        bail!(ErrorKind::InputError(format!(
            "unknown instance group {id}"
        )));
    }
    tx.commit().await?;

    changed(state, instance_ids);

    Ok(InstanceGroup {
        id,
        name: new_name.to_string(),
    })
}

/// Its instances stay, outside it.
pub async fn delete_group(state: &Desktop, id: String) -> engine::Result<()> {
    if id == FAVORITES_GROUP_ID {
        bail!(ErrorKind::InputError(
            "Favorites cannot be deleted".to_string(),
        ));
    }

    let mut tx = state.db().begin_with("BEGIN IMMEDIATE").await?;
    let instance_ids = members(&id, &mut tx).await?;
    let result = sqlx::query!("DELETE FROM instance_groups WHERE id = ?", id)
        .execute(&mut *tx)
        .await?;
    if result.rows_affected() == 0 {
        bail!(ErrorKind::InputError(format!(
            "unknown instance group {id}"
        )));
    }
    tx.commit().await?;

    changed(state, instance_ids);

    Ok(())
}

async fn members(
    group_id: &str,
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
) -> engine::Result<Vec<String>> {
    Ok(sqlx::query_scalar!(
        "SELECT instance_id FROM instance_group_memberships WHERE group_id = ?",
        group_id,
    )
    .fetch_all(&mut **tx)
    .await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_name_validation_trims_valid_names() {
        assert_eq!(validate_group_name("  My group  ").unwrap(), "My group");
    }

    #[test]
    fn group_name_validation_rejects_empty_names() {
        assert!(validate_group_name("   ").is_err());
    }

    #[test]
    fn group_name_validation_rejects_reserved_name() {
        assert!(validate_group_name("NoNe").is_err());
    }

    #[test]
    fn group_name_validation_allows_favorites() {
        assert_eq!(validate_group_name("Favorites").unwrap(), "Favorites");
    }

    #[test]
    fn group_name_validation_rejects_long_names() {
        assert!(
            validate_group_name(&"a".repeat(MAX_GROUP_NAME_LENGTH + 1))
                .is_err()
        );
    }

    #[test]
    fn membership_updates_reject_duplicate_instances() {
        let updates = vec![
            InstanceGroupMembershipUpdate {
                instance_id: "instance".to_string(),
                group_ids: vec!["first".to_string()],
            },
            InstanceGroupMembershipUpdate {
                instance_id: "instance".to_string(),
                group_ids: vec!["second".to_string()],
            },
        ];

        assert!(normalize_membership_updates(updates).is_err());
    }

    #[test]
    fn membership_updates_deduplicate_groups() {
        let updates =
            normalize_membership_updates(vec![InstanceGroupMembershipUpdate {
                instance_id: "instance".to_string(),
                group_ids: vec![
                    "first".to_string(),
                    "second".to_string(),
                    "first".to_string(),
                ],
            }])
            .unwrap();

        assert_eq!(
            updates[0].group_ids,
            vec!["first".to_string(), "second".to_string()]
        );
    }
}
