//! Deterministic Studio plugin packager CLI.

use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use studio_package::{ManifestPolicy, PackInput, PackMode, pack_bundle, parse_manifest};

fn main() -> ExitCode {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    match run(&arguments) {
        Ok(warning) => {
            if let Some(warning) = warning {
                eprintln!("{warning}");
            }
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("studio-pack: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: &[String]) -> Result<Option<&'static str>, &'static str> {
    let manifest_path = value(arguments, "--manifest")?;
    let module_path = value(arguments, "--module")?;
    let output_path = value(arguments, "--output")?;
    let development = arguments.iter().any(|argument| argument == "--dev");
    let key_path = optional_value(arguments, "--signing-key")?;
    if development == key_path.is_some() {
        return Err("select exactly one of --signing-key or --dev");
    }
    let manifest = fs::read(&manifest_path).map_err(|_| "unable to read manifest")?;
    let parsed = parse_manifest(&manifest, ManifestPolicy::default())
        .map_err(|_| "bundle manifest invalid")?;
    let root = Path::new(&manifest_path)
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let assets = parsed
        .assets
        .iter()
        .try_fold(BTreeMap::new(), |mut files, path| {
            let bytes = fs::read(root.join(path)).map_err(|_| "unable to read declared asset")?;
            files.insert(path.clone(), bytes);
            Ok(files)
        })?;
    let mode = if development {
        PackMode::DevelopmentUnsigned
    } else {
        let bytes =
            fs::read(key_path.expect("mode checked")).map_err(|_| "unable to read signing key")?;
        let seed: [u8; 32] = bytes
            .try_into()
            .map_err(|_| "signing key must be 32 raw bytes")?;
        PackMode::Signed(seed)
    };
    let bundle = pack_bundle(PackInput {
        manifest,
        module: fs::read(module_path).map_err(|_| "unable to read module")?,
        assets,
        mode,
    })
    .map_err(|_| "bundle packaging failed")?;
    fs::write(PathBuf::from(output_path), bundle).map_err(|_| "unable to write output")?;
    Ok(development.then_some("warning: unsigned development bundle"))
}

fn value(arguments: &[String], flag: &str) -> Result<String, &'static str> {
    optional_value(arguments, flag)?.ok_or("required argument missing")
}

fn optional_value(arguments: &[String], flag: &str) -> Result<Option<String>, &'static str> {
    let positions = arguments
        .iter()
        .enumerate()
        .filter(|(_, argument)| argument.as_str() == flag)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    match positions.as_slice() {
        [] => Ok(None),
        [index] => arguments
            .get(index + 1)
            .cloned()
            .map(Some)
            .ok_or("argument value missing"),
        _ => Err("argument repeated"),
    }
}
