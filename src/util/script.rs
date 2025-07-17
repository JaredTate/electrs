#[cfg(feature = "liquid")]
use crate::elements::ebcompact::*;
#[cfg(feature = "liquid")]
use elements::address as elements_address;

use crate::chain::{script, Network, Script, TxIn, TxOut};
use script::Instruction::PushBytes;
use bitcoin::{
    network::Network as BNetwork,
};

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
                } else if self.is_p2wpkh() || self.is_p2wsh() {
                    // For SegWit addresses, generate a Bitcoin address first
                    let btc_network = BNetwork::Bitcoin;
                    match bitcoin::Address::from_script(self, btc_network) {
                        Ok(addr) => {
                            // For SegWit addresses, replace "bc1" prefix with "dgb1"
                            let s = addr.to_string();
                            eprintln!("[DEBUG] to_address_str (DigiByte SegWit): btc_addr='{}', script={:?}", s, self);
                            if s.starts_with("bc1") {
                                let dgb_addr = s.replacen("bc1", "dgb1", 1);
                                eprintln!("[DEBUG] to_address_str (DigiByte SegWit): converted to dgb_addr='{}'", dgb_addr);
                                Some(dgb_addr)
                            } else {
                                Some(s)
                            }
                        }
                        Err(_) => None,
                    }
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

// -------------------------------------------------------------------
// "Reverse" parse: DGB address -> Script by base58 or bech32 prefix swap
//
//  1) If not DigiByte => parse as normal BTC
//  2) If bech32 "dgb1..." => replace w/"bc1", parse as BTC
//  3) Otherwise, try base58 decode_check(...). If first byte=30 => set to 0, if 63 => set to 5
//  4) Re-encode as BTC base58 => parse as normal BTC => get script
//  5) fallback => parse as normal BTC
//
// This avoids "legacy address base58 prefix" error for addresses like DSdh4r... .
//
// Make sure your code calls this function in your address-lookup route!
// -------------------------------------------------------------------
pub fn address_str_to_script(addr_str: &str, network: Network) -> Result<Script, String> {
    use bitcoin::address::NetworkUnchecked; // older 0.32 style
    use std::str::FromStr;

    eprintln!("[DEBUG] address_str_to_script: addr_str='{}', network={:?}", addr_str, network);

    // 1) If not DigiByte => parse as normal BTC (unchecked => checked => script)
    if network != Network::DigiByte {
        let parsed = bitcoin::Address::<NetworkUnchecked>::from_str(addr_str)
            .map_err(|e| format!("BTC parse error: {e}"))?;
        let checked = parsed.assume_checked();
        eprintln!("[DEBUG] Non-DigiByte address parsed successfully");
        return Ok(Script::from(checked.script_pubkey().into_bytes()));
    }

    // 2) If it starts with "dgb1", manually decode the bech32 and construct the script
    if addr_str.starts_with("dgb1") {
        eprintln!("[DEBUG] DigiByte bech32 address: {}", addr_str);
        
        // Manual bech32 decode for dgb1 addresses
        use bitcoin::bech32;
        
        match bech32::decode(addr_str) {
            Ok((hrp, data)) => {
                eprintln!("[DEBUG] Decoded bech32: hrp='{}'", hrp);
                
                if hrp.as_str() != "dgb" {
                    return Err(format!("Invalid HRP for DigiByte: {}", hrp));
                }
                
                if data.is_empty() {
                    return Err("Empty bech32 data".to_string());
                }
                
                // The first byte is the witness version
                let witness_version = data[0];
                
                // Convert data from 5-bit to 8-bit manually
                let mut result = Vec::new();
                let mut accumulator = 0u32;
                let mut bits = 0;
                
                for value in &data[1..] {
                    accumulator = (accumulator << 5) | (*value as u32);
                    bits += 5;
                    
                    while bits >= 8 {
                        bits -= 8;
                        result.push((accumulator >> bits) as u8);
                        accumulator &= (1 << bits) - 1;
                    }
                }
                
                // Ensure no excess bits
                if bits >= 5 || accumulator != 0 {
                    return Err("Invalid padding in bech32 data".to_string());
                }
                
                let witness_program = result;
                eprintln!("[DEBUG] Witness version: {}, program length: {}", witness_version, witness_program.len());
                
                // Construct the script based on witness version and program length
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
                } else if witness_version <= 16 {
                    // Future witness versions: OP_N + push
                    let op = if witness_version == 1 { 0x51 } else { 0x50 + witness_version };
                    Script::from([vec![op, witness_program.len() as u8], witness_program].concat())
                } else {
                    return Err(format!("Invalid witness version: {}", witness_version));
                };
                
                eprintln!("[DEBUG] DigiByte bech32 address parsed successfully, script: {:?}", script);
                return Ok(script);
            }
            Err(e) => {
                eprintln!("[DEBUG] Failed to decode dgb1 address: {:?}", e);
                return Err(format!("Invalid dgb1 address: {e}"));
            }
        }
    }

    // 3) Try base58 decode for legacy
    match bitcoin::base58::decode_check(addr_str) {
        Ok(mut raw) => {
            if !raw.is_empty() {
                eprintln!("[DEBUG] DigiByte legacy address: raw[0]={}", raw[0]);
                // If first byte=30 => rewrite to 0 (BTC P2PKH)
                if raw[0] == 30 {
                    raw[0] = 0;
                }
                // If first byte=63 => rewrite to 5 (BTC P2SH)
                else if raw[0] == 63 {
                    raw[0] = 5;
                }
                // (If not 30 or 63, it might be a fallback address or something else.)

                // re-encode as normal BTC base58
                let btc_str = bitcoin::base58::encode_check(&raw);
                eprintln!("[DEBUG] DigiByte legacy address: converted '{}' -> '{}'", addr_str, btc_str);
                // parse as normal BTC
                if let Ok(parsed_u) = bitcoin::Address::<NetworkUnchecked>::from_str(&btc_str) {
                    let checked = parsed_u.assume_checked();
                    let script = Script::from(checked.script_pubkey().into_bytes());
                    eprintln!("[DEBUG] DigiByte legacy address parsed successfully, script: {:?}", script);
                    return Ok(script);
                }
            }
        }
        Err(e) => {
            eprintln!("[DEBUG] DigiByte address base58 decode failed: {:?}", e);
            // If base58 decode fails, we just skip to fallback parse below
        }
    }

    // 4) fallback => parse as normal BTC
    eprintln!("[DEBUG] DigiByte address: trying fallback parse for '{}'", addr_str);
    let parsed = bitcoin::Address::<NetworkUnchecked>::from_str(addr_str)
        .map_err(|e| format!("Fallback parse error: {e}"))?;
    let checked = parsed.assume_checked();
    Ok(Script::from(checked.script_pubkey().into_bytes()))
}
