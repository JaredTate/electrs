#[cfg(feature = "liquid")]
use crate::elements::ebcompact::*;
#[cfg(feature = "liquid")]
use elements::address as elements_address;

use crate::chain::{script, Network, Script, TxIn, TxOut};
use script::Instruction::PushBytes;
use bitcoin::{Address as BAddress, network::Network as BNetwork};

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

#[cfg(not(feature = "liquid"))]
impl ScriptToAddr for bitcoin::Script {
    fn to_address_str(&self, network: Network) -> Option<String> {
        // Default Bitcoin-based version:
        bitcoin::Address::from_script(self, bitcoin::Network::from(network))
            .map(|s| s.to_string())
            .ok()
    }
}

#[cfg(feature = "liquid")]
impl ScriptToAddr for elements::Script {
    fn to_address_str(&self, network: Network) -> Option<String> {
        // Liquid-based version:
        elements_address::Address::from_script(self, None, network.address_params())
            .map(|a| a.to_string())
    }
}

impl ScriptToAddr for Script {
    fn to_address_str(&self, network: Network) -> Option<String> {
        match network {
            Network::DigiByte => {
                // For DigiByte addresses, manually create them from the script type
                if self.is_p2pkh() {
                    // P2PKH
                    let h160 = self.as_bytes().get(3..23)?;
                    Some(bitcoin::base58::encode_check(
                        &[30u8].iter()
                        .chain(h160.iter())
                        .copied()
                        .collect::<Vec<_>>()
                    ))
                }
                else if self.is_p2sh() {
                    // P2SH 
                    let h160 = self.as_bytes().get(2..22)?;
                    Some(bitcoin::base58::encode_check(
                        &[63u8].iter()
                        .chain(h160.iter())
                        .copied()
                        .collect::<Vec<_>>()
                    ))
                }
                else if self.is_p2wpkh() || self.is_p2wsh() {
                    // For SegWit addresses, generate a Bitcoin address first
                    let btc_network = BNetwork::Bitcoin;
                    BAddress::from_script(self, btc_network)
                        .ok()
                        .map(|addr| {
                            // For SegWit addresses, replace "bc1" prefix with "dgb1"
                            if addr.to_string().starts_with("bc1") {
                                addr.to_string().replacen("bc1", "dgb1", 1)
                            } else {
                                addr.to_string()
                            }
                        })
                }
                else {
                    None
                }
            },
            other_network => {
                // Default Bitcoin behavior
                let btc_network = BNetwork::from(other_network);
                BAddress::from_script(self, btc_network)
                    .map(|addr| addr.to_string())
                    .ok()
            }
        }
    }
}

// Returns the witnessScript in the case of p2wsh, or the redeemScript in the case of p2sh.
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

        // rust-bitcoin returns witness items as a [u8] slice, while rust-elements returns a Vec<u8>
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

// -----------------------------------------------------------------------
// ADD THIS "Option C" reverse lookup function at the bottom:
// -----------------------------------------------------------------------

/// Convert a DigiByte address string ("D...", "S...", or "dgb1...") into
/// a standard `Script` by replacing the first character/prefix with the
/// Bitcoin equivalent and parsing as a normal BTC address.
///
/// This avoids needing separate bech32/base58 crates. It's a minimal hack:
/// - "dgb1..." -> "bc1..."
/// - "D..." -> "1..."
/// - "S..." -> "3..."
/// Then we parse as Bitcoin. If none of those apply, we parse as normal BTC.
pub fn address_str_to_script(addr_str: &str, network: Network) -> Result<Script, String> {
    use std::str::FromStr;
    use bitcoin::Address as BAddress;

    // If not DigiByte, parse normally:
    if network != Network::DigiByte {
        let parsed = BAddress::from_str(addr_str).map_err(|e| e.to_string())?;
        return Ok(Script::from(parsed.assume_checked().script_pubkey().into_bytes()));
    }

    // 1) "dgb1" => replace with "bc1"
    if addr_str.starts_with("dgb1") {
        let replaced = addr_str.replacen("dgb1", "bc1", 1);
        let btc_addr = BAddress::from_str(&replaced).map_err(|e| e.to_string())?;
        return Ok(Script::from(btc_addr.assume_checked().script_pubkey().into_bytes()));
    }

    // 2) If starts with 'D', treat it as '1'
    if let Some('D') = addr_str.chars().next() {
        if addr_str.len() > 1 {
            let replaced = format!("1{}", &addr_str[1..]);
            let btc_addr = BAddress::from_str(&replaced).map_err(|e| e.to_string())?;
            return Ok(Script::from(btc_addr.assume_checked().script_pubkey().into_bytes()));
        }
    }

    // 3) If starts with 'S', treat it as '3'
    if let Some('S') = addr_str.chars().next() {
        if addr_str.len() > 1 {
            let replaced = format!("3{}", &addr_str[1..]);
            let btc_addr = BAddress::from_str(&replaced).map_err(|e| e.to_string())?;
            return Ok(Script::from(btc_addr.assume_checked().script_pubkey().into_bytes()));
        }
    }

    // 4) fallback: parse as normal BTC address
    let parsed = BAddress::from_str(addr_str).map_err(|e| e.to_string())?;
    Ok(Script::from(parsed.assume_checked().script_pubkey().into_bytes()))
}
