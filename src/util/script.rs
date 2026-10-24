#[cfg(feature = "liquid")]
use crate::elements::ebcompact::*;
#[cfg(feature = "liquid")]
use elements::address as elements_address;

use crate::chain::{script, Network, Script, TxIn, TxOut};
use script::Instruction::PushBytes;
use bitcoin::{
    bech32::{self, segwit, Hrp},
    network::Network as BNetwork,
};
use std::convert::TryFrom;

pub struct InnerScripts {
    pub redeem_script: Option<Script>,
    pub witness_script: Option<Script>,
}

pub trait ScriptToAsm: std::fmt::Debug {
    fn to_asm(&self) -> String {
        let asm = format!("{:?}", self);
        (&asm[7..asm.len() - 1]).to_string()
    }
}
impl ScriptToAsm for bitcoin::ScriptBuf {}
#[cfg(feature = "liquid")]
impl ScriptToAsm for elements::Script {}

pub trait ScriptToAddr {
    fn to_address_str(&self, network: Network) -> Option<String>;
}

// -------------------------------------------------------------------
// Default BTC-based implementation for non-liquid
// -------------------------------------------------------------------
#[cfg(not(feature = "liquid"))]
impl ScriptToAddr for bitcoin::Script {
    fn to_address_str(&self, network: Network) -> Option<String> {
        bitcoin::Address::from_script(self, bitcoin::Network::from(network))
            .map(|s| s.to_string())
            .ok()
    }
}

#[cfg(feature = "liquid")]
impl ScriptToAddr for elements::Script {
    fn to_address_str(&self, network: Network) -> Option<String> {
        elements_address::Address::from_script(self, None, network.address_params())
            .map(|a| a.to_string())
    }
}

// -------------------------------------------------------------------
// DigiByte-specific script -> DGB address (same as your original code)
// -------------------------------------------------------------------
#[cfg(not(feature = "liquid"))]
impl ScriptToAddr for Script {
    fn to_address_str(&self, network: Network) -> Option<String> {
        match network {
            Network::DigiByte => {
                // For DigiByte addresses, manually create them from the script type
                if self.is_p2pkh() {
                    // prefix=30 (0x1E)
                    let h160 = self.as_bytes().get(3..23)?;
                    Some(bitcoin::base58::encode_check(
                        &[30u8].iter()
                        .chain(h160.iter())
                        .copied()
                        .collect::<Vec<_>>()
                    ))
                } else if self.is_p2sh() {
                    // prefix=63 (0x3F)
                    let h160 = self.as_bytes().get(2..22)?;
                    Some(bitcoin::base58::encode_check(
                        &[63u8].iter()
                        .chain(h160.iter())
                        .copied()
                        .collect::<Vec<_>>()
                    ))
                } else if let Some((witness_version, witness_program)) = witness_program_from_script(self) {
                    digibyte_witness_address(witness_version, witness_program)
                } else {
                    None
                }
            },
            other_network => {
                // Default Bitcoin behavior
                let btc_network = BNetwork::from(other_network);
                bitcoin::Address::from_script(self, btc_network)
                    .map(|addr| addr.to_string())
                    .ok()
            }
        }
    }
}

fn digibyte_hrp() -> Hrp {
    Hrp::parse_unchecked("dgb")
}

fn digibyte_witness_address(witness_version: u8, witness_program: &[u8]) -> Option<String> {
    let witness_version = bech32::Fe32::try_from(witness_version).ok()?;
    segwit::encode(digibyte_hrp(), witness_version, witness_program).ok()
}

fn witness_program_from_script(script: &Script) -> Option<(u8, &[u8])> {
    let bytes = script.as_bytes();
    if bytes.len() < 4 {
        return None;
    }

    let witness_version = match bytes[0] {
        0x00 => 0,
        0x51..=0x60 => bytes[0] - 0x50,
        _ => return None,
    };

    let program_len = bytes[1] as usize;
    if bytes.len() != program_len + 2 {
        return None;
    }

    let program = &bytes[2..];
    if is_valid_witness_program(witness_version, program.len()) {
        Some((witness_version, program))
    } else {
        None
    }
}

fn is_valid_witness_program(witness_version: u8, program_len: usize) -> bool {
    if witness_version == 0 {
        program_len == 20 || program_len == 32
    } else {
        witness_version <= 16 && (2..=40).contains(&program_len)
    }
}

// -------------------------------------------------------------------
// Unchanged get_innerscripts
// -------------------------------------------------------------------
pub fn get_innerscripts(txin: &TxIn, prevout: &TxOut) -> InnerScripts {
    // Wrapped redeemScript for P2SH spends
    let redeem_script = if prevout.script_pubkey.is_p2sh() {
        if let Some(Ok(PushBytes(redeemscript))) = txin.script_sig.instructions().last() {
            #[cfg(not(feature = "liquid"))] // rust-bitcoin has a PushBytes wrapper type
            let redeemscript = redeemscript.as_bytes();
            Some(Script::from(redeemscript.to_vec()))
        } else {
            None
        }
    } else {
        None
    };

    // Wrapped witnessScript for P2WSH or P2SH-P2WSH spends
    let witness_script = if prevout.script_pubkey.is_p2wsh()
        || redeem_script.as_ref().map_or(false, |s| s.is_p2wsh())
    {
        let witness = &txin.witness;
        #[cfg(feature = "liquid")]
        let witness = &witness.script_witness;

        #[cfg(not(feature = "liquid"))]
        let wit_to_vec = Vec::from;
        #[cfg(feature = "liquid")]
        let wit_to_vec = Clone::clone;

        witness.iter().last().map(wit_to_vec).map(Script::from)
    } else {
        None
    };

    InnerScripts {
        redeem_script,
        witness_script,
    }
}

// Helper function to construct witness script
fn construct_witness_script(witness_version: u8, witness_program: Vec<u8>) -> Result<Script, String> {
    let script = if witness_version == 0 {
        if witness_program.len() == 20 {
            // P2WPKH: OP_0 + 20 bytes
            Script::from([vec![0x00, 0x14], witness_program].concat())
        } else if witness_program.len() == 32 {
            // P2WSH: OP_0 + 32 bytes
            Script::from([vec![0x00, 0x20], witness_program].concat())
        } else {
            return Err(format!("Invalid witness v0 program length: {}", witness_program.len()));
        }
    } else if witness_version <= 16 && (2..=40).contains(&witness_program.len()) {
        // Future witness versions: OP_N + push
        let op = if witness_version == 1 { 0x51 } else { 0x50 + witness_version };
        Script::from([vec![op, witness_program.len() as u8], witness_program].concat())
    } else if witness_version <= 16 {
        return Err(format!(
            "Invalid witness v{} program length: {}",
            witness_version,
            witness_program.len()
        ));
    } else {
        return Err(format!("Invalid witness version: {}", witness_version));
    };
    Ok(script)
}

// -------------------------------------------------------------------
// DigiByte address encodings per network (DigiByte Core v9.26.7
// chainparams.cpp and base58.cpp). Base58 SCRIPT_ADDRESS_OLD (5, "3...")
// is defined in chainparams but DigiByte Core does not decode it, so it
// is not accepted here either.
// -------------------------------------------------------------------
pub struct DigiByteAddressParams {
    pub pubkey_prefix: u8,
    pub script_prefix: u8,
    pub digidollar_prefix: [u8; 2],
    pub hrp: &'static str,
}

pub const DGB_MAINNET: DigiByteAddressParams = DigiByteAddressParams {
    pubkey_prefix: 30, // D...
    script_prefix: 63, // S...
    digidollar_prefix: [0x52, 0x85], // DD...
    hrp: "dgb",
};

pub const DGB_TESTNET: DigiByteAddressParams = DigiByteAddressParams {
    pubkey_prefix: 126,
    script_prefix: 140,
    digidollar_prefix: [0xb1, 0x29], // TD...
    hrp: "dgbt",
};

pub const DGB_REGTEST: DigiByteAddressParams = DigiByteAddressParams {
    pubkey_prefix: 126,
    script_prefix: 140,
    digidollar_prefix: [0xa3, 0xa4], // RD...
    hrp: "dgbrt",
};

const NOT_DIGIBYTE_ADDRESS: &str = "not a DigiByte address";

/// Parse a DigiByte address for one network. Anything else (Bitcoin,
/// Litecoin, another DigiByte network, bad checksum) is an error.
pub fn parse_digibyte_address(
    addr_str: &str,
    params: &DigiByteAddressParams,
) -> Result<Script, String> {
    let not_dgb = || NOT_DIGIBYTE_ADDRESS.to_string();

    if addr_str
        .to_ascii_lowercase()
        .starts_with(&format!("{}1", params.hrp))
    {
        // segwit::decode checks the checksum, bech32 vs bech32m and the program length
        let (hrp, witness_version, witness_program) =
            segwit::decode(addr_str).map_err(|_| not_dgb())?;
        if hrp != Hrp::parse_unchecked(params.hrp) {
            return Err(not_dgb());
        }
        let witness_version = witness_version.to_u8();
        if !is_valid_witness_program(witness_version, witness_program.len()) {
            return Err(not_dgb());
        }
        return construct_witness_script(witness_version, witness_program);
    }

    let raw = bitcoin::base58::decode_check(addr_str).map_err(|_| not_dgb())?;
    match raw.len() {
        34 if raw[..2] == params.digidollar_prefix => {
            construct_witness_script(1, raw[2..].to_vec())
        }
        21 if raw[0] == params.pubkey_prefix => {
            // P2PKH: OP_DUP OP_HASH160 <20> OP_EQUALVERIFY OP_CHECKSIG
            Ok(Script::from([&[0x76, 0xa9, 0x14][..], &raw[1..], &[0x88, 0xac]].concat()))
        }
        21 if raw[0] == params.script_prefix => {
            // P2SH: OP_HASH160 <20> OP_EQUAL
            Ok(Script::from([&[0xa9, 0x14][..], &raw[1..], &[0x87]].concat()))
        }
        _ => Err(not_dgb()),
    }
}

// -------------------------------------------------------------------
// "Reverse" parse: address string -> Script
//
//  - DigiByte: only DigiByte mainnet encodings (D..., S..., DD..., dgb1...),
//    with checksums verified. Bitcoin, Litecoin and other addresses are
//    rejected, even when they hold the same hash (WEIRD-INPUT-1).
//  - Other networks: parse as normal Bitcoin (upstream behaviour).
//
// Make sure your code calls this function in your address-lookup route!
// -------------------------------------------------------------------
pub fn address_str_to_script(addr_str: &str, network: Network) -> Result<Script, String> {
    use bitcoin::address::NetworkUnchecked; // older 0.32 style
    use std::str::FromStr;

    if network == Network::DigiByte {
        return parse_digibyte_address(addr_str, &DGB_MAINNET);
    }

    let parsed = bitcoin::Address::<NetworkUnchecked>::from_str(addr_str)
        .map_err(|e| format!("BTC parse error: {e}"))?;
    let checked = parsed.assume_checked();
    Ok(Script::from(checked.script_pubkey().into_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn script(hex: &str) -> Script {
        Script::from_hex(hex).unwrap()
    }

    #[test]
    fn digibyte_witness_v0_p2wpkh_uses_dgb_checksum() {
        let script = script("001425321626148d3a5d2c88996b4ec7a7a7fbcbc5ad");

        assert_eq!(
            script.to_address_str(Network::DigiByte).as_deref(),
            Some("dgb1qy5epvfs535a96tygn945a3a85lauh3ddu9v63y")
        );
        assert_ne!(
            script.to_address_str(Network::DigiByte).as_deref(),
            Some("dgb1qy5epvfs535a96tygn945a3a85lauh3ddt7gzf2")
        );
    }

    #[test]
    fn digibyte_witness_v0_p2wsh_uses_dgb_checksum() {
        let script =
            script("0020000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f");

        assert_eq!(
            script.to_address_str(Network::DigiByte).as_deref(),
            Some("dgb1qqqqsyqcyq5rqwzqfpg9scrgwpugpzysnzs23v9ccrydpk8qarc0slf0s5p")
        );
    }

    #[test]
    fn digibyte_witness_v1_p2tr_uses_bech32m() {
        let script =
            script("51201111111111111111111111111111111111111111111111111111111111111111");

        assert_eq!(
            script.to_address_str(Network::DigiByte).as_deref(),
            Some("dgb1pzyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zygs8lgkx2")
        );
    }

    #[test]
    fn digibyte_address_parse_validates_bech32_checksum() {
        let expected = script("001425321626148d3a5d2c88996b4ec7a7a7fbcbc5ad");

        assert_eq!(
            address_str_to_script(
                "dgb1qy5epvfs535a96tygn945a3a85lauh3ddu9v63y",
                Network::DigiByte
            )
            .unwrap(),
            expected
        );
        assert!(address_str_to_script(
            "dgb1qy5epvfs535a96tygn945a3a85lauh3ddt7gzf2",
            Network::DigiByte
        )
        .is_err());
    }

    #[test]
    fn digidollar_address_parses_to_p2tr_script() {
        assert_eq!(
            address_str_to_script(
                "DD1NQeCkofg1AyWvCWdki4N44Dihox91Xw8aYSotx7bb2cNaf6Aa",
                Network::DigiByte
            )
            .unwrap(),
            script("51201111111111111111111111111111111111111111111111111111111111111111")
        );
    }

    // WEIRD-INPUT-1: on DigiByte, only DigiByte address encodings may be accepted.
    const H160_P2PKH: &str = "76a91425321626148d3a5d2c88996b4ec7a7a7fbcbc5ad88ac";
    const H160_P2SH: &str = "a91425321626148d3a5d2c88996b4ec7a7a7fbcbc5ad87";
    const H160_P2WPKH: &str = "001425321626148d3a5d2c88996b4ec7a7a7fbcbc5ad";
    const P2WSH: &str = "0020000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
    const P2TR: &str = "51201111111111111111111111111111111111111111111111111111111111111111";

    fn assert_parses(addr: &str, network: Network, expected_hex: &str) {
        assert_eq!(
            address_str_to_script(addr, network),
            Ok(script(expected_hex)),
            "{addr} should parse"
        );
    }

    fn assert_rejected(addr: &str, network: Network) {
        let err = address_str_to_script(addr, network)
            .expect_err(&format!("{addr} must be rejected"));
        assert_eq!(err, "not a DigiByte address", "{addr}");
    }

    #[test]
    fn digibyte_mainnet_accepts_all_digibyte_address_types() {
        let net = Network::DigiByte;
        assert_parses("D8XmYRog5MvgyfEBAv2xMVhBmXcuaVjuVh", net, H160_P2PKH);
        assert_parses("SQgg31eBWKDbxxq2ym2UMdg9YB834eWHrw", net, H160_P2SH);
        assert_parses("dgb1qy5epvfs535a96tygn945a3a85lauh3ddu9v63y", net, H160_P2WPKH);
        assert_parses("DGB1QY5EPVFS535A96TYGN945A3A85LAUH3DDU9V63Y", net, H160_P2WPKH);
        assert_parses(
            "dgb1qqqqsyqcyq5rqwzqfpg9scrgwpugpzysnzs23v9ccrydpk8qarc0slf0s5p",
            net,
            P2WSH,
        );
        assert_parses(
            "dgb1pzyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zygs8lgkx2",
            net,
            P2TR,
        );
        assert_parses("DD1NQeCkofg1AyWvCWdki4N44Dihox91Xw8aYSotx7bb2cNaf6Aa", net, P2TR);
        // Real addresses from the bug report
        assert!(address_str_to_script("DSftST6gv7d1XYSvYhKXNauddAWoVnzGAS", net)
            .unwrap()
            .is_p2pkh());
        assert!(address_str_to_script("dgb1qk9z8n53s0w2z42f8g0swerszz8s6wtprcvmyvu", net)
            .unwrap()
            .is_p2wpkh());
    }

    #[test]
    fn digibyte_mainnet_rejects_bitcoin_addresses() {
        for addr in [
            // same hash160 as DSftST6gv7d1XYSvYhKXNauddAWoVnzGAS (the bug report)
            "1NXnuCA3chiizYGKp7Kxppk2k2nWBHe6jv",
            // same payloads as the DigiByte vectors above
            "14Pg1As2mx2QSf3aSL3PojXatPtcFsaiR7",
            "355gviMUKrLnXpk1ZRhzEMtX2vBKivHL7r",
            "bc1qy5epvfs535a96tygn945a3a85lauh3ddt7gzf2",
            "bc1qk9z8n53s0w2z42f8g0swerszz8s6wtpr0hlu5j",
            // random Bitcoin addresses
            "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa",
            "3J98t1WpEZ73CNmQviecrnyiWrnqRhWNLy",
            "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq",
            "tb1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx",
            "tb1qy5epvfs535a96tygn945a3a85lauh3ddpcn3je",
        ] {
            assert_rejected(addr, Network::DigiByte);
        }
    }

    #[test]
    fn digibyte_mainnet_rejects_litecoin_addresses() {
        for addr in [
            "LNcdGPArrcGThTjjcU2h5kbM6cFtK5TvMF",
            "MBHqEbmSGyCDLL1ufJhL418vMcmmjczBfq",
            "ltc1qy5epvfs535a96tygn945a3a85lauh3dd0zjx36",
        ] {
            assert_rejected(addr, Network::DigiByte);
        }
    }

    #[test]
    fn digibyte_mainnet_rejects_bad_checksums_and_garbage() {
        for addr in [
            "D8XmYRog5MvgyfEBAv2xMVhBmXcuaVjuVj",
            "SQgg31eBWKDbxxq2ym2UMdg9YB834eWHrv",
            "dgb1qk9z8n53s0w2z42f8g0swerszz8s6wtprcvmyvv",
            // v1 with a bech32 (not bech32m) checksum, v0 with a bech32m checksum
            "dgb1pzyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zygsjrc6rg",
            "dgb1qy5epvfs535a96tygn945a3a85lauh3ddfeuk5x",
            // DigiByte P2PKH version byte with a 19-byte payload
            "3kPq9w73WY2Hm6weCJuZ2ULypFBshUXfT",
            "",
            "hello",
            "<img src=x>",
            "dgb1",
        ] {
            assert_rejected(addr, Network::DigiByte);
        }
    }

    #[test]
    fn digibyte_mainnet_rejects_testnet_and_regtest_addresses() {
        for addr in [
            "skyg4rRLEgQoVGcVXC1YuXpiBxMTk4cxqU",
            "yPi7rNbNBCu4wLZhs4g1hHdj11xfkR9qsS",
            "dgbt1qy5epvfs535a96tygn945a3a85lauh3ddf0xzvf",
            "dgbrt1qy5epvfs535a96tygn945a3a85lauh3ddd7l8jt",
            "TD1SEk6vVsZqujJRrJoVQqrATFK23u7YSWWSo9wc87ub6f4ncZQH",
        ] {
            assert_rejected(addr, Network::DigiByte);
        }
    }

    #[test]
    fn digibyte_testnet_params_accept_only_testnet_addresses() {
        let parse = |addr| parse_digibyte_address(addr, &DGB_TESTNET);
        assert_eq!(parse("skyg4rRLEgQoVGcVXC1YuXpiBxMTk4cxqU"), Ok(script(H160_P2PKH)));
        assert_eq!(parse("yPi7rNbNBCu4wLZhs4g1hHdj11xfkR9qsS"), Ok(script(H160_P2SH)));
        assert_eq!(
            parse("dgbt1qy5epvfs535a96tygn945a3a85lauh3ddf0xzvf"),
            Ok(script(H160_P2WPKH))
        );
        assert_eq!(
            parse("dgbt1qqqqsyqcyq5rqwzqfpg9scrgwpugpzysnzs23v9ccrydpk8qarc0s53zl42"),
            Ok(script(P2WSH))
        );
        assert_eq!(
            parse("dgbt1pzyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zygsv89e8p"),
            Ok(script(P2TR))
        );
        assert_eq!(
            parse("TD1SEk6vVsZqujJRrJoVQqrATFK23u7YSWWSo9wc87ub6f4ncZQH"),
            Ok(script(P2TR))
        );
        for addr in [
            "D8XmYRog5MvgyfEBAv2xMVhBmXcuaVjuVh",
            "SQgg31eBWKDbxxq2ym2UMdg9YB834eWHrw",
            "dgb1qy5epvfs535a96tygn945a3a85lauh3ddu9v63y",
            "dgbrt1qy5epvfs535a96tygn945a3a85lauh3ddd7l8jt",
            "DD1NQeCkofg1AyWvCWdki4N44Dihox91Xw8aYSotx7bb2cNaf6Aa",
            "tb1qy5epvfs535a96tygn945a3a85lauh3ddpcn3je",
            "1NXnuCA3chiizYGKp7Kxppk2k2nWBHe6jv",
        ] {
            assert!(parse(addr).is_err(), "{} must be rejected on testnet", addr);
        }
    }

    #[test]
    fn digibyte_regtest_params_accept_regtest_bech32() {
        let parse = |addr| parse_digibyte_address(addr, &DGB_REGTEST);
        assert_eq!(
            parse("dgbrt1qy5epvfs535a96tygn945a3a85lauh3ddd7l8jt"),
            Ok(script(H160_P2WPKH))
        );
        assert_eq!(
            parse("RD1hoBaXyY578529rkaLAiy6DoXdgzw1s7QLCoWtWgxSrijHNnBY"),
            Ok(script(P2TR))
        );
        assert!(parse("dgbt1qy5epvfs535a96tygn945a3a85lauh3ddf0xzvf").is_err());
    }

    #[test]
    fn digibyte_legacy_addresses_are_unchanged() {
        assert_eq!(
            script("76a91425321626148d3a5d2c88996b4ec7a7a7fbcbc5ad88ac")
                .to_address_str(Network::DigiByte)
                .as_deref(),
            Some("D8XmYRog5MvgyfEBAv2xMVhBmXcuaVjuVh")
        );
        assert_eq!(
            script("a91425321626148d3a5d2c88996b4ec7a7a7fbcbc5ad87")
                .to_address_str(Network::DigiByte)
                .as_deref(),
            Some("SQgg31eBWKDbxxq2ym2UMdg9YB834eWHrw")
        );
    }
}
