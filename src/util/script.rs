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

// Helper function to convert between bit groups
fn convert_bits(data: &[u8], from_bits: u32, to_bits: u32, pad: bool) -> Option<Vec<u8>> {
    let mut acc = 0u32;
    let mut bits = 0u32;
    let mut ret = Vec::new();
    let maxv = (1 << to_bits) - 1;
    let max_acc = (1 << (from_bits + to_bits - 1)) - 1;
    
    for &value in data {
        if (value as u32) >> from_bits != 0 {
            return None;
        }
        acc = ((acc << from_bits) | (value as u32)) & max_acc;
        bits += from_bits;
        while bits >= to_bits {
            bits -= to_bits;
            ret.push(((acc >> bits) & maxv) as u8);
        }
    }
    
    if pad {
        if bits > 0 {
            ret.push(((acc << (to_bits - bits)) & maxv) as u8);
        }
    } else if bits >= from_bits || ((acc << (to_bits - bits)) & maxv) != 0 {
        return None;
    }
    
    Some(ret)
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

    // 2) If it starts with "dgb1", decode the bech32 and construct script
    if addr_str.starts_with("dgb1") {
        eprintln!("[DEBUG] DigiByte bech32 address: {}", addr_str);
        
        // Manual bech32 decode
        use bitcoin::bech32;
        
        match bech32::decode(addr_str) {
            Ok((hrp, data)) => {
                eprintln!("[DEBUG] Manual decode: hrp='{}', data_len={}", hrp, data.len());
                
                if hrp.as_str() != "dgb" {
                    return Err(format!("Invalid HRP for DigiByte: {}", hrp));
                }
                
                if data.len() < 2 {
                    return Err("Bech32 data too short".to_string());
                }
                
                // Extract witness version and convert the program
                let witness_version = data[0];
                eprintln!("[DEBUG] Witness version from data[0]: {}", witness_version);
                
                // Convert witness program from 5-bit groups to bytes
                // This is what bitcoin does internally
                let converted = convert_bits(&data[1..], 5, 8, false);
                
                match converted {
                    Some(witness_program) => {
                        eprintln!("[DEBUG] Converted witness program length: {}", witness_program.len());
                        
                        let script = if witness_version == 0 {
                            if witness_program.len() == 20 {
                                Script::from([vec![0x00, 0x14], witness_program].concat())
                            } else if witness_program.len() == 32 {
                                Script::from([vec![0x00, 0x20], witness_program].concat())
                            } else {
                                return Err(format!("Invalid witness v0 program length: {}", witness_program.len()));
                            }
                        } else if witness_version <= 16 {
                            let op = if witness_version == 1 { 0x51 } else { 0x50 + witness_version };
                            Script::from([vec![op, witness_program.len() as u8], witness_program].concat())
                        } else {
                            return Err(format!("Invalid witness version: {}", witness_version));
                        };
                        
                        eprintln!("[DEBUG] DigiByte bech32 address parsed successfully, script: {:?}", script);
                        return Ok(script);
                    }
                    None => {
                        return Err("Failed to convert witness program".to_string());
                    }
                }
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
