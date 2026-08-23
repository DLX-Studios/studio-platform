//! Deterministically generate checked-in protocol-v1 JSON Schemas.

use std::{error::Error, fs, path::Path};

use schemars::{JsonSchema, generate::SchemaSettings};
use serde_json::Value;
use studio_protocol::{
    ActionRequest, ActionResult, GuestMessage, HostEvent, MountTree, NavigationCommand, PatchBatch,
};

const SCHEMA_BASE: &str = "https://studio.local/schemas/protocol-v1";

fn schema_document<T: JsonSchema>(filename: &str) -> Result<String, Box<dyn Error>> {
    let generator = SchemaSettings::draft2020_12().into_generator();
    let mut schema = generator.into_root_schema_for::<T>();
    schema.ensure_object().insert(
        "$id".to_owned(),
        Value::String(format!("{SCHEMA_BASE}/{filename}")),
    );
    let mut document = serde_json::to_string_pretty(&schema)?;
    document.push('\n');
    Ok(document)
}

fn write_schema<T: JsonSchema>(directory: &Path, filename: &str) -> Result<(), Box<dyn Error>> {
    fs::write(directory.join(filename), schema_document::<T>(filename)?)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let repository_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let schemas_root = repository_root.join("protocol/schemas");
    let output = schemas_root.join("protocol-v1");
    let staging = schemas_root.join(".protocol-v1.generate");

    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    fs::create_dir_all(&staging)?;

    write_schema::<ActionRequest>(&staging, "action-request.schema.json")?;
    write_schema::<ActionResult>(&staging, "action-result.schema.json")?;
    write_schema::<GuestMessage>(&staging, "guest-message.schema.json")?;
    write_schema::<HostEvent>(&staging, "host-event.schema.json")?;
    write_schema::<MountTree>(&staging, "mount-tree.schema.json")?;
    write_schema::<NavigationCommand>(&staging, "navigation-command.schema.json")?;
    write_schema::<PatchBatch>(&staging, "patch-batch.schema.json")?;

    if output.exists() {
        fs::remove_dir_all(&output)?;
    }
    fs::rename(staging, output)?;
    Ok(())
}
