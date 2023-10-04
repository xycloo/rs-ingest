use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use crate::SupportedNetwork;

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

const PREDEFINED_PUBNET_CONFIG: &str = r#"
LOG_COLOR=true
LOG_FILE_PATH=""
HTTP_PORT=0
PUBLIC_HTTP_PORT=false

NETWORK_PASSPHRASE="Public Global Stellar Network ; September 2015"

#EXPERIMENTAL_PRECAUTION_DELAY_META=true
DATABASE="sqlite3://stellar.db"
PEER_PORT=11725

UNSAFE_QUORUM=true

[[HOME_DOMAINS]]
HOME_DOMAIN="stellar.org"
QUALITY="MEDIUM"


[[VALIDATORS]]
NAME="sdf_1"
HOME_DOMAIN="stellar.org"
PUBLIC_KEY="GCGB2S2KGYARPVIA37HYZXVRM2YZUEXA6S33ZU5BUDC6THSB62LZSTYH"
ADDRESS="core-live-a.stellar.org:11625"
HISTORY="curl -sf https://history.stellar.org/prd/core-live/core_live_001/{0} -o {1}"

"#;

const PREDEFINED_TESTNET_CONFIG: &str = r#"
LOG_COLOR=true
LOG_FILE_PATH=""
HTTP_PORT=0
PUBLIC_HTTP_PORT=false

NETWORK_PASSPHRASE="Test SDF Network ; September 2015"

#EXPERIMENTAL_PRECAUTION_DELAY_META=true
DATABASE="sqlite3://stellar.db"
PEER_PORT=11725

UNSAFE_QUORUM=true

[[HOME_DOMAINS]]
HOME_DOMAIN="testnet.stellar.org"
QUALITY="MEDIUM"

[[VALIDATORS]]
NAME="sdf_testnet_1"
HOME_DOMAIN="testnet.stellar.org"
PUBLIC_KEY="GDKXE2OZMJIPOSLNA6N6F2BVCI3O777I2OOC4BV7VOYUEHYX7RTRYA7Y"
ADDRESS="core-testnet1.stellar.org"
HISTORY="curl -sf http://history.stellar.org/prd/core-testnet/core_testnet_001/{0} -o {1}"

"#;

pub fn generate_predefined_cfg(path: &str, network: SupportedNetwork) {
    match fs::create_dir(path) {
        Ok(_) => println!("Directory created successfully."),
        Err(err) => {
            if err.kind() != std::io::ErrorKind::AlreadyExists {
                eprintln!("Error creating directory: {}", err);
                panic!();
            }
        }
    }

    let mut cfg =
        File::create(Path::new(path).join("stellar-core.cfg")).expect("cannot create file");

    match network {
        SupportedNetwork::Futurenet => {
            cfg.write_all(PREDEFINED_FUTURENET_CONFIG.as_bytes())
                .expect("cannot write to file");
        }

        SupportedNetwork::Pubnet => {
            cfg.write_all(PREDEFINED_PUBNET_CONFIG.as_bytes())
                .expect("cannot write to file");
        }

        SupportedNetwork::Testnet => {
            cfg.write_all(PREDEFINED_TESTNET_CONFIG.as_bytes())
                .expect("cannot write to file")
        }
    }
}
