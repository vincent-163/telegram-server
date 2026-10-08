use anyhow::{bail, Context, Result};
use clap::{Arg, ArgAction, Command};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use telegram_server::botapi::{router, AppState};
use telegram_server::config::Config;
use telegram_server::connection;
use telegram_server::mtproto::RsaKeyPair;
use telegram_server::store::Store;

fn config_from(matches: &clap::ArgMatches) -> Result<Config> {
    let dc_id = matches
        .get_one::<String>("dc-id")
        .and_then(|v| v.parse().ok())
        .unwrap_or(2);
    let public_ip = matches.get_one::<String>("public-ip").cloned();
    let public_ipv6 = matches.get_one::<String>("public-ipv6").cloned();
    let mtproto_bind = matches
        .get_one::<String>("mtproto-bind")
        .cloned()
        .unwrap_or_else(|| "0.0.0.0".into());
    let mtproto_port = matches
        .get_one::<String>("mtproto-port")
        .and_then(|v| v.parse().ok())
        .unwrap_or(443);
    let bot_api_bind = matches
        .get_one::<String>("bot-bind")
        .cloned()
        .unwrap_or_else(|| "0.0.0.0".into());
    let bot_api_port = matches
        .get_one::<String>("bot-port")
        .and_then(|v| v.parse().ok())
        .unwrap_or(8081);
    let db_path = matches
        .get_one::<String>("db")
        .cloned()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("telegram-server.db"));
    let rsa_key_path = matches
        .get_one::<String>("rsa-key")
        .cloned()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("telegram-server-rsa.key"));
    let admin_password = matches.get_one::<String>("admin-password").cloned();
    let login_code = matches.get_one::<String>("login-code").cloned();
    Ok(Config::new(
        dc_id,
        public_ip,
        public_ipv6,
        mtproto_bind,
        mtproto_port,
        bot_api_bind,
        bot_api_port,
        db_path,
        rsa_key_path,
        admin_password,
        login_code,
    ))
}

fn server_command() -> Command {
    Command::new("telegram-server")
        .version(env!("CARGO_PKG_VERSION"))
        .arg(
            Arg::new("db")
                .long("db")
                .default_value("telegram-server.db"),
        )
        .arg(
            Arg::new("rsa-key")
                .long("rsa-key")
                .default_value("telegram-server-rsa.key"),
        )
        .arg(Arg::new("dc-id").long("dc-id").default_value("2"))
        .arg(Arg::new("public-ip").long("public-ip"))
        .arg(Arg::new("public-ipv6").long("public-ipv6"))
        .arg(
            Arg::new("mtproto-bind")
                .long("mtproto-bind")
                .default_value("0.0.0.0"),
        )
        .arg(
            Arg::new("mtproto-port")
                .long("mtproto-port")
                .default_value("443"),
        )
        .arg(
            Arg::new("bot-bind")
                .long("bot-bind")
                .default_value("0.0.0.0"),
        )
        .arg(Arg::new("bot-port").long("bot-port").default_value("8081"))
        .arg(Arg::new("admin-password").long("admin-password"))
        .arg(Arg::new("login-code").long("login-code"))
        .subcommand(
            Command::new("admin")
                .subcommand(
                    Command::new("create-user")
                        .arg(Arg::new("phone").long("phone").required(true))
                        .arg(Arg::new("first-name").long("first-name").default_value(""))
                        .arg(Arg::new("last-name").long("last-name").default_value(""))
                        .arg(Arg::new("username").long("username").default_value(""))
                        .arg(Arg::new("admin").long("admin").action(ArgAction::SetTrue)),
                )
                .subcommand(
                    Command::new("create-bot")
                        .arg(Arg::new("phone").long("phone").required(true))
                        .arg(
                            Arg::new("first-name")
                                .long("first-name")
                                .default_value("Bot"),
                        )
                        .arg(Arg::new("username").long("username").required(true))
                        .arg(Arg::new("bot-token").long("bot-token").required(true)),
                )
                .subcommand(Command::new("genkey").arg(Arg::new("out").long("out"))),
        )
}

fn set_key_permissions(path: &std::path::Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    let _ = path;
    Ok(())
}

fn load_or_create_rsa_key(path: &std::path::Path) -> Result<RsaKeyPair> {
    match std::fs::read_to_string(path) {
        Ok(contents) => RsaKeyPair::from_hex(contents.trim())
            .with_context(|| format!("parse RSA key {}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let key = RsaKeyPair::generate();
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create(true).truncate(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options
                .open(path)
                .with_context(|| format!("write RSA key {}", path.display()))?;
            use std::io::Write;
            file.write_all(key.to_hex().as_bytes())
                .with_context(|| format!("write RSA key {}", path.display()))?;
            drop(file);
            set_key_permissions(path)?;
            tracing::info!("created persistent MTProto RSA key at {}", path.display());
            Ok(key)
        }
        Err(e) => Err(e).with_context(|| format!("read RSA key {}", path.display())),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
    let matches = server_command().get_matches();
    let cfg = Arc::new(config_from(&matches)?);
    let store = Store::open(&cfg.db_path).context("open database")?;
    if let Some(("admin", sub)) = matches.subcommand() {
        match sub.subcommand() {
            Some(("create-user", a)) => {
                let user = store.create_user(
                    a.get_one::<String>("phone").unwrap(),
                    a.get_one::<String>("first-name").unwrap(),
                    a.get_one::<String>("last-name").unwrap(),
                    a.get_one::<String>("username").unwrap(),
                    false,
                    a.get_flag("admin"),
                )?;
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "id": user.id, "phone": user.phone, "username": user.username, "admin": user.admin
                    }))?
                );
            }
            Some(("create-bot", a)) => {
                let user = store.create_user(
                    a.get_one::<String>("phone").unwrap(),
                    a.get_one::<String>("first-name").unwrap(),
                    "",
                    a.get_one::<String>("username").unwrap(),
                    true,
                    false,
                )?;
                let token = a.get_one::<String>("bot-token").unwrap();
                store.set_bot_token(user.id, token)?;
                // Re-read so the printed row reflects the token just stored.
                let user = store
                    .get_user(user.id)?
                    .ok_or_else(|| anyhow::anyhow!("bot vanished"))?;
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "id": user.id, "username": user.username, "bot_token": user.bot_token
                    }))?
                );
            }
            Some(("genkey", a)) => {
                let out = a
                    .get_one::<String>("out")
                    .cloned()
                    .unwrap_or_else(|| cfg.rsa_key_path.to_string_lossy().into_owned());
                let key = RsaKeyPair::generate();
                let mut options = std::fs::OpenOptions::new();
                options.write(true).create(true).truncate(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    options.mode(0o600);
                }
                let mut file = options.open(&out)?;
                use std::io::Write;
                file.write_all(key.to_hex().as_bytes())?;
                drop(file);
                set_key_permissions(Path::new(&out))?;
                println!("{}", key.public_pem());
            }
            _ => bail!("unknown admin command"),
        }
        return Ok(());
    }

    let rsa = load_or_create_rsa_key(&cfg.rsa_key_path)?;
    tracing::info!(
        "MTProto RSA fingerprint 0x{:016x}",
        rsa.fingerprint() as u64
    );
    let state = AppState {
        store: store.clone(),
        cfg: cfg.clone(),
        rsa: rsa.clone(),
        http_mtproto: Arc::new(telegram_server::http_mtproto::HttpMtProtoState::new()),
    };
    let bot_listener = tokio::net::TcpListener::bind((cfg.bot_api_bind.as_str(), cfg.bot_api_port))
        .await
        .with_context(|| format!("bind bot api {}:{}", cfg.bot_api_bind, cfg.bot_api_port))?;
    tracing::info!(
        "Bot API listening on {}:{}",
        cfg.bot_api_bind,
        cfg.bot_api_port
    );
    let http = axum::serve(bot_listener, router(state));
    let mtproto = connection::serve(cfg.clone(), store, rsa);
    let bot_http = async { http.await.map_err(|e| anyhow::anyhow!("{}", e)) };
    tokio::try_join!(mtproto, bot_http)?;
    Ok(())
}
