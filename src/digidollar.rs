use crate::chain::{Script, Transaction, TxOut};
use bitcoin::hex::DisplayHex;

const OP_0: u8 = 0x00;
const OP_PUSHDATA1: u8 = 0x4c;
const OP_PUSHDATA2: u8 = 0x4d;
const OP_PUSHDATA4: u8 = 0x4e;
const OP_1NEGATE: u8 = 0x4f;
const OP_1: u8 = 0x51;
const OP_16: u8 = 0x60;
const OP_RETURN: u8 = 0x6a;
const OP_DIGIDOLLAR: u8 = 0xbb;
const OP_ORACLE: u8 = 0xbf;

const DD_TX_VERSION: u32 = 0x0d1d0770;
const DD_VERSION_MASK: u32 = 0x0000ffff;
const DD_TYPE_MASK: u32 = 0xff000000;
const DD_FLAGS_MASK: u32 = 0x00ff0000;
const ORACLE_MIN_PRICE_MICRO_USD: u64 = 100;
const ORACLE_MAX_PRICE_MICRO_USD: u64 = 100_000_000;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct DigiDollarTxInfo {
    pub tx_marker: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_code: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flags: Option<u8>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub metadata: Vec<DigiDollarOutputInfo>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub oracle_bundles: Vec<DigiDollarOutputInfo>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct DigiDollarOutputInfo {
    pub kind: String,
    pub valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vout: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_code: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount_cents: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amounts_cents: Option<Vec<i64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dd_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lock_height: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lock_tier: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_xonly_pubkey: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oracle_version: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bitmap_len: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub participation_bitmap: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub participant_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub epoch: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_micro_usd: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aggregate_sig: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
enum ScriptItem {
    Op(u8),
    Push(Vec<u8>),
    SmallInt(i64),
}

pub fn parse_script(script: &Script) -> Option<DigiDollarOutputInfo> {
    let bytes = script.as_bytes();
    if bytes.first().copied() != Some(OP_RETURN) {
        return None;
    }

    if bytes.get(1).copied() == Some(OP_ORACLE) {
        return Some(match read_items_after_op_return(bytes) {
            Ok(items) => parse_oracle_bundle(&items),
            Err(e) => invalid_output("oracle_bundle", e),
        });
    }

    let items = match read_items_after_op_return(bytes) {
        Ok(items) => items,
        Err(_) => return None,
    };

    match items.first() {
        Some(ScriptItem::Push(marker)) if marker == b"DD" => Some(parse_dd_metadata(&items)),
        Some(ScriptItem::Op(OP_DIGIDOLLAR)) => Some(parse_legacy_amount(&items)),
        _ => None,
    }
}

pub fn analyze_transaction(version: u32, outputs: &[TxOut]) -> Option<DigiDollarTxInfo> {
    let tx_marker = (version & DD_VERSION_MASK) == (DD_TX_VERSION & DD_VERSION_MASK);
    let type_code = tx_marker.then_some(((version & DD_TYPE_MASK) >> 24) as u8);
    let flags = tx_marker.then_some(((version & DD_FLAGS_MASK) >> 16) as u8);
    let tx_type = type_code.and_then(tx_type_name).map(str::to_owned);

    let mut metadata = Vec::new();
    let mut oracle_bundles = Vec::new();
    for (index, output) in outputs.iter().enumerate() {
        if let Some(mut parsed) = parse_script(&output.script_pubkey) {
            parsed.vout = Some(index as u32);
            if parsed.kind == "oracle_bundle" {
                oracle_bundles.push(parsed);
            } else {
                metadata.push(parsed);
            }
        }
    }

    if !tx_marker && metadata.is_empty() && oracle_bundles.is_empty() {
        return None;
    }

    Some(DigiDollarTxInfo {
        tx_marker,
        tx_type,
        type_code,
        flags,
        metadata,
        oracle_bundles,
    })
}

pub fn analyze_transaction_outputs(version: u32, outputs: &[TxOut]) -> Vec<Option<DigiDollarOutputInfo>> {
    let mut parsed: Vec<Option<DigiDollarOutputInfo>> = outputs
        .iter()
        .map(|output| parse_script(&output.script_pubkey))
        .collect();

    let tx_marker = (version & DD_VERSION_MASK) == (DD_TX_VERSION & DD_VERSION_MASK);
    let type_code = ((version & DD_TYPE_MASK) >> 24) as u8;
    if !tx_marker || !matches!(type_code, 1..=3) {
        return parsed;
    }

    let metadata = parsed
        .iter()
        .filter_map(|item| item.as_ref())
        .find(|info| info.valid && info.type_code == Some(type_code));

    let Some(metadata) = metadata else {
        return parsed;
    };

    let amounts = match type_code {
        1 | 3 => metadata.amount_cents.map(|amount| vec![amount]),
        2 => metadata.amounts_cents.clone(),
        _ => None,
    };
    let Some(amounts) = amounts else {
        return parsed;
    };

    let dd_output_indexes: Vec<usize> = outputs
        .iter()
        .enumerate()
        .filter_map(|(index, output)| is_dd_token_output(output).then_some(index))
        .collect();

    if dd_output_indexes.len() != amounts.len() {
        return parsed;
    }

    for (index, amount) in dd_output_indexes.into_iter().zip(amounts.into_iter()) {
        let mut output = base_output("token_output", true);
        output.vout = Some(index as u32);
        output.tx_type = tx_type_name(type_code).map(str::to_owned);
        output.type_code = Some(type_code);
        output.amount_cents = Some(amount);
        output.dd_address = digidollar_address_from_script(&outputs[index].script_pubkey);
        parsed[index] = Some(output);
    }

    parsed
}

pub fn transaction_has_digidollar_activity(tx: &Transaction) -> bool {
    analyze_transaction(tx.version.0 as u32, &tx.output).is_some()
}

fn parse_dd_metadata(items: &[ScriptItem]) -> DigiDollarOutputInfo {
    let type_code = match items.get(1).map(script_num) {
        Some(Ok(n)) if (0..=u8::MAX as i64).contains(&n) => n as u8,
        Some(Ok(_)) => return invalid_output("digidollar_metadata", "transaction type out of range"),
        Some(Err(e)) => return invalid_output("digidollar_metadata", e),
        None => return invalid_output("digidollar_metadata", "missing transaction type"),
    };

    match type_code {
        1 => parse_mint_metadata(items, type_code),
        2 => parse_transfer_metadata(items, type_code),
        3 => parse_redeem_metadata(items, type_code),
        _ => {
            let mut output = invalid_output("digidollar_metadata", "unsupported DigiDollar transaction type");
            output.type_code = Some(type_code);
            output
        }
    }
}

fn parse_mint_metadata(items: &[ScriptItem], type_code: u8) -> DigiDollarOutputInfo {
    let mut output = base_output("mint_metadata", true);
    output.tx_type = tx_type_name(type_code).map(str::to_owned);
    output.type_code = Some(type_code);

    output.amount_cents = match items.get(2).map(script_num) {
        Some(Ok(n)) if n > 0 => Some(n),
        Some(Ok(_)) => return invalid_output("mint_metadata", "mint amount must be positive"),
        Some(Err(e)) => return invalid_output("mint_metadata", e),
        None => return invalid_output("mint_metadata", "missing mint amount"),
    };
    output.lock_height = match items.get(3).map(script_num) {
        Some(Ok(n)) if n > 0 => Some(n),
        Some(Ok(_)) => return invalid_output("mint_metadata", "lock height must be positive"),
        Some(Err(e)) => return invalid_output("mint_metadata", e),
        None => return invalid_output("mint_metadata", "missing lock height"),
    };
    output.lock_tier = match items.get(4).map(script_num) {
        Some(Ok(n)) if (0..=9).contains(&n) => Some(n),
        Some(Ok(_)) => return invalid_output("mint_metadata", "lock tier must be 0..9"),
        Some(Err(e)) => return invalid_output("mint_metadata", e),
        None => return invalid_output("mint_metadata", "missing lock tier"),
    };
    output.owner_xonly_pubkey = match items.get(5).and_then(push_bytes) {
        Some(key) if key.len() == 32 => Some(key.as_hex().to_string()),
        Some(_) => return invalid_output("mint_metadata", "owner x-only pubkey must be 32 bytes"),
        None => return invalid_output("mint_metadata", "missing owner x-only pubkey"),
    };
    if items.len() != 6 {
        return invalid_output("mint_metadata", "unexpected trailing mint metadata");
    }
    output
}

fn parse_transfer_metadata(items: &[ScriptItem], type_code: u8) -> DigiDollarOutputInfo {
    let mut amounts = Vec::new();
    for item in items.iter().skip(2) {
        match script_num(item) {
            Ok(n) if n > 0 => amounts.push(n),
            Ok(_) => return invalid_output("transfer_metadata", "transfer amount must be positive"),
            Err(e) => return invalid_output("transfer_metadata", e),
        }
    }
    if amounts.is_empty() {
        return invalid_output("transfer_metadata", "missing transfer amounts");
    }

    let mut output = base_output("transfer_metadata", true);
    output.tx_type = tx_type_name(type_code).map(str::to_owned);
    output.type_code = Some(type_code);
    output.amount_cents = Some(amounts.iter().sum());
    output.amounts_cents = Some(amounts);
    output
}

fn parse_redeem_metadata(items: &[ScriptItem], type_code: u8) -> DigiDollarOutputInfo {
    let mut output = base_output("redeem_metadata", true);
    output.tx_type = tx_type_name(type_code).map(str::to_owned);
    output.type_code = Some(type_code);
    output.amount_cents = match items.get(2).map(script_num) {
        Some(Ok(n)) if n > 0 => Some(n),
        Some(Ok(_)) => return invalid_output("redeem_metadata", "redeem change amount must be positive"),
        Some(Err(e)) => return invalid_output("redeem_metadata", e),
        None => return invalid_output("redeem_metadata", "missing redeem change amount"),
    };
    if items.len() != 3 {
        return invalid_output("redeem_metadata", "unexpected trailing redeem metadata");
    }
    output
}

fn parse_legacy_amount(items: &[ScriptItem]) -> DigiDollarOutputInfo {
    let mut output = base_output("legacy_amount", true);
    output.amount_cents = match items.get(1).and_then(push_bytes) {
        Some(data) if data.len() == 8 => {
            let mut bytes = [0u8; 8];
            bytes.copy_from_slice(data);
            Some(u64::from_le_bytes(bytes) as i64)
        }
        Some(_) => return invalid_output("legacy_amount", "legacy amount must be 8 bytes"),
        None => return invalid_output("legacy_amount", "missing legacy amount"),
    };
    if items.len() != 2 {
        return invalid_output("legacy_amount", "unexpected trailing legacy metadata");
    }
    output
}

fn parse_oracle_bundle(items: &[ScriptItem]) -> DigiDollarOutputInfo {
    if !matches!(items.first(), Some(ScriptItem::Op(OP_ORACLE))) {
        return invalid_output("oracle_bundle", "missing OP_ORACLE marker");
    }

    let version = match items.get(1).map(script_num) {
        Some(Ok(3)) => 3u8,
        Some(Ok(_)) => return invalid_output("oracle_bundle", "unsupported oracle bundle version"),
        Some(Err(e)) => return invalid_output("oracle_bundle", e),
        None => return invalid_output("oracle_bundle", "missing oracle bundle version"),
    };
    let data = match items.get(2).and_then(push_bytes) {
        Some(data) => data,
        None => return invalid_output("oracle_bundle", "missing oracle v0x03 payload"),
    };
    if items.len() != 3 {
        return invalid_output("oracle_bundle", "unexpected trailing oracle payload");
    }

    let mut pos = 0;
    let bitmap_len = match data.get(pos).copied() {
        Some(0) => return invalid_output("oracle_bundle", "oracle bitmap length cannot be zero"),
        Some(n) => n as usize,
        None => return invalid_output("oracle_bundle", "missing oracle bitmap length"),
    };
    pos += 1;
    let expected = 1 + bitmap_len + 4 + 8 + 8 + 64;
    if data.len() != expected {
        return invalid_output("oracle_bundle", "oracle v0x03 payload length mismatch");
    }

    let bitmap = &data[pos..pos + bitmap_len];
    pos += bitmap_len;

    let mut epoch_bytes = [0u8; 4];
    epoch_bytes.copy_from_slice(&data[pos..pos + 4]);
    let epoch = u32::from_le_bytes(epoch_bytes);
    pos += 4;

    let mut price_bytes = [0u8; 8];
    price_bytes.copy_from_slice(&data[pos..pos + 8]);
    let price = u64::from_le_bytes(price_bytes);
    pos += 8;
    if !(ORACLE_MIN_PRICE_MICRO_USD..=ORACLE_MAX_PRICE_MICRO_USD).contains(&price) {
        return invalid_output("oracle_bundle", "oracle price out of range");
    }

    let mut timestamp_bytes = [0u8; 8];
    timestamp_bytes.copy_from_slice(&data[pos..pos + 8]);
    let timestamp = i64::from_le_bytes(timestamp_bytes);
    pos += 8;

    let sig = &data[pos..pos + 64];

    let mut output = base_output("oracle_bundle", true);
    output.oracle_version = Some(version);
    output.bitmap_len = Some(bitmap_len as u8);
    output.participation_bitmap = Some(bitmap.as_hex().to_string());
    output.participant_count = Some(bitmap.iter().map(|b| b.count_ones()).sum());
    output.epoch = Some(epoch);
    output.price_micro_usd = Some(price);
    output.timestamp = Some(timestamp);
    output.aggregate_sig = Some(sig.as_hex().to_string());
    output
}

pub fn digidollar_address_from_script(script: &Script) -> Option<String> {
    let program = p2tr_program(script)?;
    let mut payload = Vec::with_capacity(34);
    payload.extend_from_slice(&[0x52, 0x85]);
    payload.extend_from_slice(program);
    Some(bitcoin::base58::encode_check(&payload))
}

fn is_dd_token_output(output: &TxOut) -> bool {
    output.value.to_sat() == 0 && p2tr_program(&output.script_pubkey).is_some()
}

fn p2tr_program(script: &Script) -> Option<&[u8]> {
    let bytes = script.as_bytes();
    if bytes.len() == 34 && bytes[0] == OP_1 && bytes[1] == 32 {
        Some(&bytes[2..34])
    } else {
        None
    }
}

fn read_items_after_op_return(bytes: &[u8]) -> Result<Vec<ScriptItem>, String> {
    if bytes.first().copied() != Some(OP_RETURN) {
        return Err("script is not OP_RETURN".to_string());
    }

    let mut items = Vec::new();
    let mut pos = 1;
    while pos < bytes.len() {
        let opcode = bytes[pos];
        pos += 1;
        match opcode {
            OP_0 => items.push(ScriptItem::SmallInt(0)),
            OP_1NEGATE => items.push(ScriptItem::SmallInt(-1)),
            OP_1..=OP_16 => items.push(ScriptItem::SmallInt((opcode - OP_1 + 1) as i64)),
            1..=75 => {
                let len = opcode as usize;
                if pos + len > bytes.len() {
                    return Err("pushdata length exceeds script length".to_string());
                }
                items.push(ScriptItem::Push(bytes[pos..pos + len].to_vec()));
                pos += len;
            }
            OP_PUSHDATA1 => {
                let len = *bytes
                    .get(pos)
                    .ok_or_else(|| "missing OP_PUSHDATA1 length".to_string())?
                    as usize;
                pos += 1;
                if pos + len > bytes.len() {
                    return Err("OP_PUSHDATA1 length exceeds script length".to_string());
                }
                items.push(ScriptItem::Push(bytes[pos..pos + len].to_vec()));
                pos += len;
            }
            OP_PUSHDATA2 => {
                if pos + 2 > bytes.len() {
                    return Err("missing OP_PUSHDATA2 length".to_string());
                }
                let len = u16::from_le_bytes([bytes[pos], bytes[pos + 1]]) as usize;
                pos += 2;
                if pos + len > bytes.len() {
                    return Err("OP_PUSHDATA2 length exceeds script length".to_string());
                }
                items.push(ScriptItem::Push(bytes[pos..pos + len].to_vec()));
                pos += len;
            }
            OP_PUSHDATA4 => {
                if pos + 4 > bytes.len() {
                    return Err("missing OP_PUSHDATA4 length".to_string());
                }
                let len = u32::from_le_bytes([
                    bytes[pos],
                    bytes[pos + 1],
                    bytes[pos + 2],
                    bytes[pos + 3],
                ]) as usize;
                pos += 4;
                if pos + len > bytes.len() {
                    return Err("OP_PUSHDATA4 length exceeds script length".to_string());
                }
                items.push(ScriptItem::Push(bytes[pos..pos + len].to_vec()));
                pos += len;
            }
            op => items.push(ScriptItem::Op(op)),
        }
    }
    Ok(items)
}

fn script_num(item: &ScriptItem) -> Result<i64, String> {
    match item {
        ScriptItem::SmallInt(n) => Ok(*n),
        ScriptItem::Push(data) => decode_script_num(data),
        ScriptItem::Op(op) => Err(format!("opcode 0x{op:02x} is not a script number")),
    }
}

fn push_bytes(item: &ScriptItem) -> Option<&[u8]> {
    match item {
        ScriptItem::Push(data) => Some(data),
        _ => None,
    }
}

fn decode_script_num(data: &[u8]) -> Result<i64, String> {
    if data.len() > 8 {
        return Err("script number exceeds 8 bytes".to_string());
    }
    if !is_minimally_encoded(data) {
        return Err("script number is not minimally encoded".to_string());
    }
    if data.is_empty() {
        return Ok(0);
    }

    let negative = data[data.len() - 1] & 0x80 != 0;
    let mut value = 0i64;
    for (i, byte) in data.iter().enumerate() {
        let byte = if i == data.len() - 1 {
            byte & 0x7f
        } else {
            *byte
        };
        value |= (byte as i64) << (8 * i);
    }
    Ok(if negative { -value } else { value })
}

fn is_minimally_encoded(data: &[u8]) -> bool {
    if data.is_empty() {
        return true;
    }
    if (data[data.len() - 1] & 0x7f) != 0 {
        return true;
    }
    data.len() > 1 && (data[data.len() - 2] & 0x80) != 0
}

fn tx_type_name(type_code: u8) -> Option<&'static str> {
    match type_code {
        1 => Some("mint"),
        2 => Some("transfer"),
        3 => Some("redeem"),
        _ => None,
    }
}

fn base_output(kind: &str, valid: bool) -> DigiDollarOutputInfo {
    DigiDollarOutputInfo {
        kind: kind.to_string(),
        valid,
        error: None,
        vout: None,
        tx_type: None,
        type_code: None,
        amount_cents: None,
        amounts_cents: None,
        dd_address: None,
        lock_height: None,
        lock_tier: None,
        owner_xonly_pubkey: None,
        oracle_version: None,
        bitmap_len: None,
        participation_bitmap: None,
        participant_count: None,
        epoch: None,
        price_micro_usd: None,
        timestamp: None,
        aggregate_sig: None,
    }
}

fn invalid_output(kind: &str, error: impl Into<String>) -> DigiDollarOutputInfo {
    let mut output = base_output(kind, false);
    output.error = Some(error.into());
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::{OutPoint, Script, Sequence, Transaction, TxIn};
    use bitcoin::{absolute::LockTime, transaction::Version, Amount, Witness};

    const OP_RETURN: u8 = 0x6a;
    const OP_ORACLE: u8 = 0xbf;

    fn push(data: &[u8]) -> Vec<u8> {
        let mut script = if data.len() < 0x4c {
            vec![data.len() as u8]
        } else if data.len() <= u8::MAX as usize {
            vec![0x4c, data.len() as u8]
        } else {
            panic!("test push too large");
        };
        script.extend_from_slice(data);
        script
    }

    fn script_num(n: i64) -> Vec<u8> {
        if n == 0 {
            return push(&[]);
        }

        let negative = n < 0;
        let mut abs = if negative { -n } else { n } as u64;
        let mut data = Vec::new();
        while abs > 0 {
            data.push((abs & 0xff) as u8);
            abs >>= 8;
        }
        if data.last().map_or(false, |b| b & 0x80 != 0) {
            data.push(if negative { 0x80 } else { 0x00 });
        } else if negative {
            let last = data.len() - 1;
            data[last] |= 0x80;
        }
        push(&data)
    }

    fn script(bytes: Vec<u8>) -> Script {
        Script::from(bytes)
    }

    fn transfer_script() -> Script {
        let mut bytes = vec![OP_RETURN];
        bytes.extend(push(b"DD"));
        bytes.extend(script_num(2));
        bytes.extend(script_num(100));
        bytes.extend(script_num(250));
        script(bytes)
    }

    fn p2tr_script(byte: u8) -> Script {
        let mut bytes = vec![OP_1, 32];
        bytes.extend_from_slice(&[byte; 32]);
        script(bytes)
    }

    #[test]
    fn parses_mint_metadata() {
        let mut bytes = vec![OP_RETURN];
        bytes.extend(push(b"DD"));
        bytes.extend(script_num(1));
        bytes.extend(script_num(12345));
        bytes.extend(script_num(23_900_000));
        bytes.extend(script_num(3));
        bytes.extend(push(&[0x22; 32]));

        let parsed = parse_script(&script(bytes)).expect("DD metadata");
        assert_eq!(parsed.kind, "mint_metadata");
        assert!(parsed.valid);
        assert_eq!(parsed.tx_type.as_deref(), Some("mint"));
        assert_eq!(parsed.amount_cents, Some(12345));
        assert_eq!(parsed.lock_height, Some(23_900_000));
        assert_eq!(parsed.lock_tier, Some(3));
        assert_eq!(
            parsed.owner_xonly_pubkey.as_deref(),
            Some("2222222222222222222222222222222222222222222222222222222222222222")
        );
    }

    #[test]
    fn parses_transfer_without_output_count() {
        let transfer = transfer_script();
        let parsed = parse_script(&transfer).expect("DD metadata");
        assert_eq!(parsed.kind, "transfer_metadata");
        assert!(parsed.valid);
        assert_eq!(parsed.tx_type.as_deref(), Some("transfer"));
        assert_eq!(parsed.amount_cents, Some(350));
        assert_eq!(parsed.amounts_cents, Some(vec![100, 250]));
    }

    #[test]
    fn parses_oracle_v03_bundle() {
        let mut payload = vec![5, 0x7f, 0, 0, 0, 0];
        payload.extend_from_slice(&596_768u32.to_le_bytes());
        payload.extend_from_slice(&3_230u64.to_le_bytes());
        payload.extend_from_slice(&1_780_000_000u64.to_le_bytes());
        payload.extend_from_slice(&[0xaa; 64]);

        let mut bytes = vec![OP_RETURN, OP_ORACLE];
        bytes.extend(push(&[0x03]));
        bytes.extend(push(&payload));

        let parsed = parse_script(&script(bytes)).expect("oracle bundle");
        assert_eq!(parsed.kind, "oracle_bundle");
        assert!(parsed.valid);
        assert_eq!(parsed.oracle_version, Some(3));
        assert_eq!(parsed.bitmap_len, Some(5));
        assert_eq!(parsed.participant_count, Some(7));
        assert_eq!(parsed.epoch, Some(596_768));
        assert_eq!(parsed.price_micro_usd, Some(3_230));
        assert_eq!(parsed.timestamp, Some(1_780_000_000));
    }

    #[test]
    fn analyzes_transaction_version_and_metadata_outputs() {
        let output = TxOut {
            value: Amount::from_sat(0),
            script_pubkey: transfer_script(),
        };

        let info = analyze_transaction(0x0200_0770, &[output]).expect("DD tx info");
        assert!(info.tx_marker);
        assert_eq!(info.tx_type.as_deref(), Some("transfer"));
        assert_eq!(info.type_code, Some(2));
        assert_eq!(info.flags, Some(0));
        assert_eq!(info.metadata.len(), 1);
        assert_eq!(info.metadata[0].vout, Some(0));
        assert_eq!(info.metadata[0].amounts_cents, Some(vec![100, 250]));
    }

    #[test]
    fn encodes_digidollar_address_from_p2tr_output() {
        assert_eq!(
            digidollar_address_from_script(&p2tr_script(0x11)).as_deref(),
            Some("DD1NQeCkofg1AyWvCWdki4N44Dihox91Xw8aYSotx7bb2cNaf6Aa")
        );
    }

    #[test]
    fn maps_transfer_amounts_to_zero_value_p2tr_outputs() {
        let outputs = vec![
            TxOut {
                value: Amount::from_sat(0),
                script_pubkey: p2tr_script(0x11),
            },
            TxOut {
                value: Amount::from_sat(0),
                script_pubkey: p2tr_script(0x22),
            },
            TxOut {
                value: Amount::from_sat(1000),
                script_pubkey: p2tr_script(0x33),
            },
            TxOut {
                value: Amount::from_sat(0),
                script_pubkey: transfer_script(),
            },
        ];

        let parsed = analyze_transaction_outputs(0x0200_0770, &outputs);
        assert_eq!(parsed[0].as_ref().map(|info| info.kind.as_str()), Some("token_output"));
        assert_eq!(parsed[0].as_ref().and_then(|info| info.amount_cents), Some(100));
        assert_eq!(
            parsed[0].as_ref().and_then(|info| info.dd_address.as_deref()),
            Some("DD1NQeCkofg1AyWvCWdki4N44Dihox91Xw8aYSotx7bb2cNaf6Aa")
        );
        assert_eq!(parsed[1].as_ref().and_then(|info| info.amount_cents), Some(250));
        assert_eq!(
            parsed[1].as_ref().and_then(|info| info.dd_address.as_deref()),
            Some("DD1Vvb241PdnKaqZFEojEzKRi1o1f8PLFzB3tCHYQnEvPSfaLNaf")
        );
        assert!(parsed[2].is_none());
        assert_eq!(parsed[3].as_ref().map(|info| info.kind.as_str()), Some("transfer_metadata"));
    }

    #[test]
    fn does_not_partially_map_transfer_amount_mismatches() {
        let outputs = vec![
            TxOut {
                value: Amount::from_sat(0),
                script_pubkey: p2tr_script(0x11),
            },
            TxOut {
                value: Amount::from_sat(0),
                script_pubkey: p2tr_script(0x22),
            },
            TxOut {
                value: Amount::from_sat(0),
                script_pubkey: {
                    let mut bytes = vec![OP_RETURN];
                    bytes.extend(push(b"DD"));
                    bytes.extend(script_num(2));
                    bytes.extend(script_num(100));
                    script(bytes)
                },
            },
        ];

        let parsed = analyze_transaction_outputs(0x0200_0770, &outputs);
        assert!(parsed[0].is_none());
        assert!(parsed[1].is_none());
        assert_eq!(parsed[2].as_ref().map(|info| info.kind.as_str()), Some("transfer_metadata"));
    }

    #[test]
    fn detects_digidollar_transaction_activity() {
        let tx = Transaction {
            version: Version(0x0200_0770),
            lock_time: LockTime::ZERO,
            input: vec![TxIn {
                previous_output: OutPoint::null(),
                script_sig: Script::new(),
                sequence: Sequence::MAX,
                witness: Witness::default(),
            }],
            output: vec![TxOut {
                value: Amount::from_sat(0),
                script_pubkey: transfer_script(),
            }],
        };

        assert!(transaction_has_digidollar_activity(&tx));
    }

    #[test]
    fn malformed_dd_metadata_is_returned_as_invalid() {
        let mut bytes = vec![OP_RETURN];
        bytes.extend(push(b"DD"));
        bytes.extend(script_num(1));
        bytes.extend(script_num(100));

        let parsed = parse_script(&script(bytes)).expect("DD metadata");
        assert_eq!(parsed.kind, "mint_metadata");
        assert!(!parsed.valid);
        assert_eq!(parsed.error.as_deref(), Some("missing lock height"));
    }
}
