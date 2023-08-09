use std::fs::File;
use std::fs;
use std::io::Write;
use std::path::Path;

const PREDEFINED_FUTURENET_CONFIG: &str = r#"
# captive core config for futurenet
LOG_COLOR=true
LOG_FILE_PATH=""
HTTP_PORT=0
PUBLIC_HTTP_PORT=false

NETWORK_PASSPHRASE="Test SDF Future Network ; October 2022"

#EXPERIMENTAL_PRECAUTION_DELAY_META=true
DATABASE="sqlite3://stellar.db"
PEER_PORT=11725

UNSAFE_QUORUM=true


# Stellar Futurenet validators
[[HOME_DOMAINS]]
HOME_DOMAIN="futurenet.stellar.org"
QUALITY="MEDIUM"

[[VALIDATORS]]
NAME="sdf_futurenet_1"
HOME_DOMAIN="futurenet.stellar.org"
PUBLIC_KEY="GBRIF2N52GVN3EXBBICD5F4L5VUFXK6S6VOUCF6T2DWPLOLGWEPPYZTF"
ADDRESS="core-live-futurenet.stellar.org"
HISTORY="curl -sf http://history-futurenet.stellar.org/{0} -o {1}"
"#;

pub fn generate_predefined_cfg(path: &str) {
    match fs::create_dir(path) {
        Ok(_) => println!("Directory created successfully."),
        Err(err) => {
            if err.kind() != std::io::ErrorKind::AlreadyExists {
                eprintln!("Error creating directory: {}", err);
                panic!();
            }
        }
    }

    let mut cfg = File::create(Path::new(path).join("stellar-core.cfg")).expect("cannot create file");
    cfg.write_all(PREDEFINED_FUTURENET_CONFIG.as_bytes()).expect("cannot write to file");
}