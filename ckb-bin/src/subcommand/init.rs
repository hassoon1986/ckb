use std::fs;
use std::path::PathBuf;

use crate::helper::prompt;
use ckb_app_config::{ExitCode, InitArgs};
use ckb_core::script::ScriptHashType;
use ckb_db::{db::RocksDB, DBConfig, Error};
use ckb_resource::{
    Resource, TemplateContext, AVAILABLE_SPECS, CKB_CONFIG_FILE_NAME,
    CODE_HASH_SECP256K1_BLAKE160_SIGHASH_ALL, DEFAULT_SPEC, MINER_CONFIG_FILE_NAME,
    SPEC_DEV_FILE_NAME,
};
use ckb_script::Runner;

const SECP256K1_BLAKE160_SIGHASH_ALL_ARG_LEN: usize = 20 * 2 + 2; // 42 = 20 x 2 + prefix 0x

fn check_db_compatibility(path: PathBuf) {
    if path.exists() {
        let config = DBConfig {
            path: path.clone(),
            ..Default::default()
        };
        if let Some(Error::DBError(err_msg)) = RocksDB::open_with_error(&config, 1).err() {
            if err_msg.contains("the database version is not matched") {
                let input =
                    prompt(format!("Database is not incompatible, remove {:?}? ", path).as_str());

                if ["y", "Y"].contains(&input.trim()) {
                    if let Some(e) = fs::remove_dir_all(path).err() {
                        eprintln!("{}", e);
                    }
                }
            }
        }
    }
}

pub fn init(args: InitArgs) -> Result<(), ExitCode> {
    let mut args = args;

    if args.list_chains {
        for spec in AVAILABLE_SPECS {
            println!("{}", spec);
        }
        return Ok(());
    }

    let exported = Resource::exported_in(&args.root_dir);
    if !args.force && exported {
        eprintln!("Config files already exists, use --force to overwrite.");

        if args.interactive {
            let input = prompt("Overwrite config file now? ");

            if !["y", "Y"].contains(&input.trim()) {
                return Err(ExitCode::Failure);
            }
        } else {
            return Err(ExitCode::Failure);
        }
    }

    if args.interactive {
        let data_dir = args.root_dir.join("data");
        let db_path = data_dir.join("db");
        let indexer_db_path = data_dir.join("indexer_db");

        check_db_compatibility(db_path);
        check_db_compatibility(indexer_db_path);

        let _block_assembler_code_hash = prompt("code hash: ");
        let _args = prompt("args: ");
        let _data = prompt("data: ");
        let _hash_type = prompt("hash_type: ");

        args.block_assembler_code_hash = Some(_block_assembler_code_hash.trim().to_string());

        args.block_assembler_args = _args
            .trim()
            .split_whitespace()
            .map(|s| s.to_string())
            .collect::<Vec<String>>();

        args.block_assembler_data = Some(_data.trim().to_string());

        match serde_plain::from_str::<ScriptHashType>(_hash_type.trim()).ok() {
            Some(hash_type) => args.block_assembler_hash_type = hash_type,
            None => eprintln!("Invalid block assembler hash type"),
        }
    }

    let runner = Runner::default().to_string();
    let default_hash = format!("{:#x}", CODE_HASH_SECP256K1_BLAKE160_SIGHASH_ALL);
    let block_assembler_code_hash = args.block_assembler_code_hash.as_ref().or_else(|| {
        if !args.block_assembler_args.is_empty() {
            Some(&default_hash)
        } else {
            None
        }
    });

    let block_assembler = match block_assembler_code_hash {
        Some(hash) => {
            if default_hash != *hash {
                eprintln!(
                    "WARN: the default secp256k1 code hash is `{}`, you are using `{}`.\n\
                     It will require `ckb run --ba-advanced` to enable this block assembler",
                    default_hash, hash
                );
            } else if args.block_assembler_args.len() != 1
                || args.block_assembler_args[0].len() != SECP256K1_BLAKE160_SIGHASH_ALL_ARG_LEN
            {
                eprintln!(
                    "WARN: the block assembler arg is not a valid secp256k1 pubkey hash.\n\
                     It will require `ckb run --ba-advanced` to enable this block assembler"
                );
            }
            format!(
                "[block_assembler]\n\
                 code_hash = \"{}\"\n\
                 args = [ \"{}\" ]\n\
                 data = \"{}\"\n\
                 hash_type = \"{}\"",
                hash,
                args.block_assembler_args.join("\", \""),
                args.block_assembler_data
                    .unwrap_or_else(|| "0x".to_string()),
                serde_plain::to_string(&args.block_assembler_hash_type).unwrap(),
            )
        }
        None => {
            eprintln!("WARN: mining feature is disabled because of lacking the block assembler config options");
            format!(
                "# secp256k1_blake160_sighash_all example:\n\
                 # [block_assembler]\n\
                 # code_hash = \"{:#x}\"\n\
                 # args = [ \"ckb cli blake160 <compressed-pubkey>\" ]\n\
                 # data = \"A 0x-prefixed hex string\"\n\
                 # hash_type = \"Hash type, could be Data or Type\"",
                CODE_HASH_SECP256K1_BLAKE160_SIGHASH_ALL,
            )
        }
    };

    let context = TemplateContext {
        spec: &args.chain,
        rpc_port: &args.rpc_port,
        p2p_port: &args.p2p_port,
        log_to_file: args.log_to_file,
        log_to_stdout: args.log_to_stdout,
        runner: &runner,
        block_assembler: &block_assembler,
    };

    println!(
        "{} CKB directory in {}",
        if !exported {
            "Initialized"
        } else {
            "Reinitialized"
        },
        args.root_dir.display()
    );

    println!("create {}", CKB_CONFIG_FILE_NAME);
    Resource::bundled_ckb_config().export(&context, &args.root_dir)?;
    println!("create {}", MINER_CONFIG_FILE_NAME);
    Resource::bundled_miner_config().export(&context, &args.root_dir)?;

    if args.chain == DEFAULT_SPEC {
        println!("create {}", SPEC_DEV_FILE_NAME);
        Resource::bundled(SPEC_DEV_FILE_NAME.to_string()).export(&context, &args.root_dir)?;
    }

    Ok(())
}
