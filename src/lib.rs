use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::Deref;

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct CompactSize {
    pub value: u64,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum BitcoinError {
    InsufficientBytes,
    InvalidFormat,
}

impl CompactSize {
    pub fn new(value: u64) -> Self {
        CompactSize { value }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        match self.value {
            0..=252 => bytes.push(self.value as u8),
            253..=65535 => {
                bytes.push(0xFD);
                bytes.extend_from_slice(&(self.value as u16).to_le_bytes());
            }
            65536..=4294967295 => {
                bytes.push(0xFE);
                bytes.extend_from_slice(&(self.value as u32).to_le_bytes());
            }
            _ => {
                bytes.push(0xFF);
                bytes.extend_from_slice(&self.value.to_le_bytes());
            }
        }
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        if bytes.is_empty() {
            return Err(BitcoinError::InsufficientBytes);
        }

        match bytes[0] {
            0..=252 => Ok((CompactSize::new(bytes[0] as u64), 1)),
            0xFD => {
                if bytes.len() < 3 {
                    return Err(BitcoinError::InsufficientBytes);
                }
                let value = u16::from_le_bytes([bytes[1], bytes[2]]) as u64;
                Ok((CompactSize::new(value), 3))
            }
            0xFE => {
                if bytes.len() < 5 {
                    return Err(BitcoinError::InsufficientBytes);
                }
                let value = u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as u64;
                Ok((CompactSize::new(value), 5))
            }
            0xFF => {
                if bytes.len() < 9 {
                    return Err(BitcoinError::InsufficientBytes);
                }
                let value = u64::from_le_bytes([
                    bytes[1], bytes[2], bytes[3], bytes[4],
                    bytes[5], bytes[6], bytes[7], bytes[8],
                ]);
                Ok((CompactSize::new(value), 9))
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Txid(pub [u8; 32]);

impl Serialize for Txid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let hex = hex::encode(self.0);
        serializer.serialize_str(&hex)
    }
}

impl<'de> Deserialize<'de> for Txid {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let hex_str = String::deserialize(deserializer)?;
        let bytes = hex::decode(hex_str)
            .map_err(serde::de::Error::custom)?;
        if bytes.len() != 32 {
            return Err(serde::de::Error::custom("Invalid txid length"));
        }
        let mut array = [0u8; 32];
        array.copy_from_slice(&bytes);
        Ok(Txid(array))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct OutPoint {
    pub txid: Txid,
    pub vout: u32,
}

impl OutPoint {
    pub fn new(txid: [u8; 32], vout: u32) -> Self {
        OutPoint {
            txid: Txid(txid),
            vout,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(36);
        bytes.extend_from_slice(&self.txid.0);
        bytes.extend_from_slice(&self.vout.to_le_bytes());
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        if bytes.len() < 36 {
            return Err(BitcoinError::InsufficientBytes);
        }
        let mut txid = [0u8; 32];
        txid.copy_from_slice(&bytes[0..32]);
        let vout = u32::from_le_bytes([
            bytes[32], bytes[33], bytes[34], bytes[35]
        ]);
        Ok((OutPoint::new(txid, vout), 36))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Script {
    pub bytes: Vec<u8>,
}

impl Script {
    pub fn new(bytes: Vec<u8>) -> Self {
        Script { bytes }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        let length = CompactSize::new(self.bytes.len() as u64);
        bytes.extend(length.to_bytes());
        bytes.extend(&self.bytes);
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        let (length, consumed) = CompactSize::from_bytes(bytes)?;
        let script_len = length.value as usize;
        if bytes.len() < consumed + script_len {
            return Err(BitcoinError::InsufficientBytes);
        }
        let script_bytes = bytes[consumed..consumed + script_len].to_vec();
        Ok((Script::new(script_bytes), consumed + script_len))
    }
}

impl Deref for Script {
    type Target = Vec<u8>;
    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct TransactionInput {
    pub previous_output: OutPoint,
    pub script_sig: Script,
    pub sequence: u32,
}

impl TransactionInput {
    pub fn new(previous_output: OutPoint, script_sig: Script, sequence: u32) -> Self {
        TransactionInput {
            previous_output,
            script_sig,
            sequence,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend(self.previous_output.to_bytes());
        bytes.extend(self.script_sig.to_bytes());
        bytes.extend_from_slice(&self.sequence.to_le_bytes());
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        let mut cursor = 0;
        let (previous_output, consumed) = OutPoint::from_bytes(&bytes[cursor..])?;
        cursor += consumed;
        
        let (script_sig, consumed) = Script::from_bytes(&bytes[cursor..])?;
        cursor += consumed;
        
        if bytes.len() < cursor + 4 {
            return Err(BitcoinError::InsufficientBytes);
        }
        let sequence = u32::from_le_bytes([
            bytes[cursor],
            bytes[cursor + 1],
            bytes[cursor + 2],
            bytes[cursor + 3],
        ]);
        cursor += 4;
        
        Ok((TransactionInput::new(previous_output, script_sig, sequence), cursor))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct BitcoinTransaction {
    pub version: u32,
    pub inputs: Vec<TransactionInput>,
    pub lock_time: u32,
}

impl BitcoinTransaction {
    pub fn new(version: u32, inputs: Vec<TransactionInput>, lock_time: u32) -> Self {
        BitcoinTransaction {
            version,
            inputs,
            lock_time,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        
        // Version (4 bytes LE)
        bytes.extend_from_slice(&self.version.to_le_bytes());
        
        // Number of inputs as CompactSize
        let input_count = CompactSize::new(self.inputs.len() as u64);
        bytes.extend(input_count.to_bytes());
        
        // Serialize each input
        for input in &self.inputs {
            bytes.extend(input.to_bytes());
        }
        
        // Lock time (4 bytes LE)
        bytes.extend_from_slice(&self.lock_time.to_le_bytes());
        
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        let mut cursor = 0;
        
        // Read version (4 bytes LE)
        if bytes.len() < 4 {
            return Err(BitcoinError::InsufficientBytes);
        }
        let version = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        cursor += 4;
        
        // Read input count
        let (input_count, consumed) = CompactSize::from_bytes(&bytes[cursor..])?;
        cursor += consumed;
        
        // Read inputs
        let mut inputs = Vec::with_capacity(input_count.value as usize);
        for _ in 0..input_count.value {
            let (input, consumed) = TransactionInput::from_bytes(&bytes[cursor..])?;
            inputs.push(input);
            cursor += consumed;
        }
        
        // Read lock_time
        if bytes.len() < cursor + 4 {
            return Err(BitcoinError::InsufficientBytes);
        }
        let lock_time = u32::from_le_bytes([
            bytes[cursor],
            bytes[cursor + 1],
            bytes[cursor + 2],
            bytes[cursor + 3]
        ]);
        cursor += 4;
        
        Ok((BitcoinTransaction::new(version, inputs, lock_time), cursor))
    }
}

impl fmt::Display for BitcoinTransaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Version: {}", self.version)?;
        
        for (i, input) in self.inputs.iter().enumerate() {
            writeln!(f, "Input {}", i)?;
            writeln!(f, "  Previous Output Txid: {:?}", input.previous_output.txid)?;
            writeln!(f, "  Previous Output Vout: {}", input.previous_output.vout)?;
            writeln!(f, "  ScriptSig Length: {}", input.script_sig.bytes.len())?;
            writeln!(f, "  ScriptSig: {:?}", input.script_sig.bytes)?;
            writeln!(f, "  Sequence: {:#x}", input.sequence)?;
        }
        
        writeln!(f, "Lock Time: {}", self.lock_time)
    }
}