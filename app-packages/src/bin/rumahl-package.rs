use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use rumahl_app_packages::{
    GeneratedKey, KeyStore, PackageManifest, PackageVerifier, TrustStore, trust_store_json,
    write_key_store,
};
use serde_json::{Value, json};

const HELP: &str = "\
Usage:
  rumahl-package keygen --key-id ID --publisher PUBLISHER --keystore FILE [--trust-store FILE]
  rumahl-package sign --package DIR --manifest TEMPLATE.json --keystore FILE
  rumahl-package verify --package DIR --trust-store FILE
";

fn main() -> ExitCode {
    if std::env::args().any(|argument| argument == "--help" || argument == "-h") {
        print!("{HELP}");
        return ExitCode::SUCCESS;
    }
    match run(std::env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("rumahl-package: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: Vec<String>) -> Result<(), Box<dyn Error>> {
    let mut arguments = arguments.into_iter();
    let command = arguments.next().ok_or("missing command")?;
    let options = parse_options(arguments)?;

    match command.as_str() {
        "keygen" => keygen(&options),
        "sign" => sign(&options),
        "verify" => verify(&options),
        other => Err(format!("unknown command '{other}'").into()),
    }
}

fn keygen(options: &HashMap<String, String>) -> Result<(), Box<dyn Error>> {
    let key_id = required(options, "--key-id")?;
    let publisher = required(options, "--publisher")?;
    let keystore_path = PathBuf::from(required(options, "--keystore")?);

    let generated = GeneratedKey::generate(key_id)?;
    write_key_store(&keystore_path, &generated.key_store())?;

    let entry = trust_store_json(key_id, publisher, &generated.public_key());
    println!("Wrote signing key {key_id} to {}.", keystore_path.display());

    if let Some(path) = options.get("--trust-store") {
        let path = Path::new(path);
        merge_trust_store(path, entry["keys"][0].clone())?;
        println!("Added {key_id} to {}.", path.display());
    } else {
        let document = json!({ "formatVersion": 1, "keys": [entry["keys"][0].clone()] });
        println!(
            "Trust store entry:\n{}",
            serde_json::to_string_pretty(&document)?
        );
    }
    Ok(())
}

fn sign(options: &HashMap<String, String>) -> Result<(), Box<dyn Error>> {
    let package_dir = PathBuf::from(required(options, "--package")?);
    let manifest_path = required(options, "--manifest")?;
    let keystore_path = required(options, "--keystore")?;

    let template_bytes = fs::read(manifest_path)?;
    let template = PackageManifest::from_bytes(&template_bytes)?;

    let keystore_bytes = fs::read(keystore_path)?;
    let keystore = KeyStore::parse(&keystore_bytes)?;
    let signer = keystore.signer()?;

    signer.sign(&package_dir, &template)?;
    println!(
        "Signed {} with key {}.",
        package_dir.display(),
        signer.key_id()
    );
    Ok(())
}

fn verify(options: &HashMap<String, String>) -> Result<(), Box<dyn Error>> {
    let package_dir = PathBuf::from(required(options, "--package")?);
    let trust_store_path = required(options, "--trust-store")?;

    let trust_store_bytes = fs::read(trust_store_path)?;
    let trust_store = TrustStore::from_bytes(&trust_store_bytes)?;
    let verified = PackageVerifier::new(trust_store).verify(&package_dir)?;

    println!(
        "Verified {} file(s) from publisher {} with key {}.",
        verified.files().len(),
        verified.publisher_id().as_str(),
        verified.signing_key_id()
    );
    Ok(())
}

fn merge_trust_store(path: &Path, entry: Value) -> Result<(), Box<dyn Error>> {
    let mut document: Value = if path.exists() {
        let bytes = fs::read(path)?;
        TrustStore::from_bytes(&bytes)?;
        serde_json::from_slice(&bytes)?
    } else {
        json!({ "formatVersion": 1, "keys": [] })
    };

    let keys = document
        .get_mut("keys")
        .and_then(Value::as_array_mut)
        .ok_or("trust store document has no keys array")?;
    if keys
        .iter()
        .any(|existing| existing.get("keyId") == entry.get("keyId"))
    {
        return Err(format!(
            "key id {} is already present",
            entry.get("keyId").and_then(Value::as_str).unwrap_or("")
        )
        .into());
    }
    keys.push(entry);

    fs::write(path, serde_json::to_vec_pretty(&document)?)?;
    Ok(())
}

fn required<'a>(
    options: &'a HashMap<String, String>,
    name: &str,
) -> Result<&'a str, Box<dyn Error>> {
    options
        .get(name)
        .map(String::as_str)
        .ok_or_else(|| format!("missing required option {name}").into())
}

fn parse_options(
    arguments: impl IntoIterator<Item = String>,
) -> Result<HashMap<String, String>, Box<dyn Error>> {
    let mut options = HashMap::new();
    let mut arguments = arguments.into_iter();
    while let Some(name) = arguments.next() {
        if !name.starts_with("--") {
            return Err(format!("unexpected argument '{name}'").into());
        }
        let value = arguments
            .next()
            .ok_or_else(|| format!("option {name} is missing its value"))?;
        if options.insert(name.clone(), value).is_some() {
            return Err(format!("option {name} was provided more than once").into());
        }
    }
    Ok(options)
}
