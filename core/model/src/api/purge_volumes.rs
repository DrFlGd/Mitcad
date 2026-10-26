// SPDX-License-Identifier: MIT
//! Commands and queries of purge volumes on filament changes (mitcad#705,
//! `document/purge_volumes.rs`, `crate::print_purge`): the pairs of
//! materials set by hand, the table of the bodies' filament slots and a
//! colour's default values. See `commands.md`, "Purge volumes".

use serde::Deserialize;
use serde_json::{Value, json};

use super::ApiError;
use super::dispatch::named_enum;
use crate::document::{Document, PurgeChange, purge_defaults_json};
use crate::ids::BodyUid;
use crate::kernel::Kernel;

named_enum! {
    #[derive(Debug, Deserialize)]
    #[serde(tag = "cmd", rename_all = "snake_case", deny_unknown_fields)]
    pub(crate) enum PurgeCommand {
        "set_purge_volumes" => SetPurgeVolumes {
            /// `{from, to, volume}` per pair of materials (ids); a `volume`
            /// of null or left out resets the pair to its computed volume.
            volumes: Vec<PurgeChange>,
        },
    }
}

/// The `purge_volumes` query.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PurgeVolumesQuery {
    /// The bodies printed, in order; all the design's when left out.
    #[serde(default)]
    bodies: Vec<BodyUid>,
}

/// The `purge_defaults` query.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PurgeDefaultsQuery {
    /// `#RRGGBB`; none for mid grey.
    #[serde(default)]
    color: Option<String>,
}

impl<K: Kernel> Document<K> {
    pub(crate) fn run_purge_command(&mut self, command: PurgeCommand) -> Result<Value, ApiError> {
        Ok(match command {
            PurgeCommand::SetPurgeVolumes { volumes } => {
                self.set_purge_volumes(&volumes)?;
                json!({})
            }
        })
    }

    pub(super) fn purge_volumes_query(&self, query: &PurgeVolumesQuery) -> Result<Value, ApiError> {
        Ok(self.purge_volumes_json(&query.bodies)?)
    }
}

pub(super) fn purge_defaults_query(query: &PurgeDefaultsQuery) -> Result<Value, ApiError> {
    Ok(purge_defaults_json(query.color.as_deref())?)
}
