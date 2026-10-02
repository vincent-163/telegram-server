use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub dc_id: i32,
    public_ip_override: Option<String>,
    public_ipv6_override: Option<String>,
    pub mtproto_bind: String,
    pub mtproto_port: u16,
    pub bot_api_bind: String,
    pub bot_api_port: u16,
    pub db_path: PathBuf,
    pub rsa_key_path: PathBuf,
    pub admin_password: Option<String>,
    pub login_code: Option<String>,
}

impl Config {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        dc_id: i32,
        public_ip: Option<String>,
        public_ipv6: Option<String>,
        mtproto_bind: String,
        mtproto_port: u16,
        bot_api_bind: String,
        bot_api_port: u16,
        db_path: PathBuf,
        rsa_key_path: PathBuf,
        admin_password: Option<String>,
        login_code: Option<String>,
    ) -> Self {
        Self {
            dc_id,
            public_ip_override: public_ip,
            public_ipv6_override: public_ipv6,
            mtproto_bind,
            mtproto_port,
            bot_api_bind,
            bot_api_port,
            db_path,
            rsa_key_path,
            admin_password,
            login_code,
        }
    }

    pub fn public_ip(&self, connected: std::net::SocketAddr) -> String {
        self.public_ip_override
            .clone()
            .unwrap_or_else(|| connected.ip().to_string())
    }

    pub fn public_ipv6(&self, connected: std::net::SocketAddr) -> Option<String> {
        self.public_ipv6_override.clone().or_else(|| {
            if connected.is_ipv6() {
                Some(connected.ip().to_string())
            } else {
                None
            }
        })
    }
}
