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

    // 2) If it starts with "dgb1", do "bc1" replacement -> parse as BTC bech32
    if addr_str.starts_with("dgb1") {
        let replaced = addr_str.replacen("dgb1", "bc1", 1);
        eprintln!("[DEBUG] DigiByte bech32 address: replaced '{}' -> '{}'", addr_str, replaced);
        let parsed = bitcoin::Address::<NetworkUnchecked>::from_str(&replaced)
            .map_err(|e| format!("Bech32 parse error: {e}"))?;
        let checked = parsed.assume_checked();
        let script = Script::from(checked.script_pubkey().into_bytes());
        eprintln!("[DEBUG] DigiByte bech32 address parsed successfully, script: {:?}", script);
        return Ok(script);
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
