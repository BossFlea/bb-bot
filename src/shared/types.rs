use std::fmt::Display;

use anyhow::{Context as _, Result, anyhow, bail};

use crate::error::UserError;

pub struct BitSet {
    pub data: Vec<u8>,
}

impl BitSet {
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self { data: bytes }
    }

    pub fn from_indexes(ids: &[u8]) -> Self {
        let max = ids.iter().max().copied().unwrap_or(0) as usize;

        let mut bitset = Self {
            data: vec![0; max / 8 + 1],
        };

        for &id in ids {
            bitset.set(id as usize, true);
        }

        bitset
    }

    #[allow(dead_code)] // currently unused
    pub fn get(&self, bit_index: usize) -> bool {
        let byte_index = bit_index / 8;
        if byte_index >= self.data.len() {
            return false;
        }
        let bit_offset = bit_index % 8;

        let byte = self.data[byte_index];
        let mask = 1u8 << bit_offset;

        byte & mask != 0
    }

    /// Returns how many ones there are before the target; for the nth set bit this returns n-1
    #[allow(dead_code)] // currently unused
    pub fn relative_index_of(&self, bit_index: usize) -> Option<usize> {
        let byte_index = bit_index / 8;
        if byte_index >= self.data.len() {
            return None;
        }
        let bit_offset = bit_index % 8;

        let byte = self.data[byte_index];
        let mask = 1u8 << bit_offset;

        let mut count: usize = self.data[..byte_index]
            .iter()
            .map(|b| b.count_ones() as usize)
            .sum();

        // all ones on lesser bits than target
        let before_mask = mask - 1;

        count += (byte & before_mask).count_ones() as usize;

        Some(count)
    }

    pub fn get_all_set(&self) -> Vec<usize> {
        let mut set_indexes = Vec::new();

        for (byte_index, &byte) in self.data.iter().enumerate() {
            let mut current_byte = byte;

            while current_byte != 0 {
                let zeroes = current_byte.trailing_zeros() as usize;
                let bit_index = byte_index * 8 + zeroes;

                set_indexes.push(bit_index);

                // clear the bit
                current_byte &= current_byte - 1;
            }
        }

        set_indexes
    }

    pub fn set(&mut self, bit_index: usize, value: bool) {
        let byte_index = bit_index / 8;
        let bit_offset = bit_index % 8;

        if byte_index >= self.data.len() {
            self.data.resize(byte_index + 1, 0);
        }

        let byte = &mut self.data[byte_index];
        let mask = 1u8 << bit_offset;

        if value {
            *byte |= mask;
        } else {
            *byte &= !mask;
        }
    }

    #[allow(dead_code)]
    pub fn toggle(&mut self, bit_index: usize) {
        let byte_index = bit_index / 8;
        let bit_offset = bit_index % 8;

        if byte_index >= self.data.len() {
            self.data.resize(byte_index + 1, 0);
        }

        let byte = &mut self.data[byte_index];
        let mask = 1u8 << bit_offset;

        *byte ^= mask;
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BingoKind {
    Normal = 0,
    Extreme = 1,
    Secret = 2,
}

impl BingoKind {
    pub const ALL: &[BingoKind; 3] = &[BingoKind::Normal, BingoKind::Extreme, BingoKind::Secret];

    pub fn from_u8(int: u8) -> Self {
        match int {
            1 => Self::Extreme,
            2 => Self::Secret,
            _ => Self::Normal,
        }
    }

    pub fn as_prefix(&self) -> &str {
        match self {
            BingoKind::Normal => "",
            BingoKind::Extreme => "Extreme ",
            BingoKind::Secret => "Secret ",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Bingo {
    pub kind_specific_id: u8,
    pub kind: BingoKind,
    pub unique_id: Option<u8>,
}

impl Display for Bingo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let prefix = self.kind.as_prefix();
        write!(f, "{}Bingo #{}", prefix, self.kind_specific_id + 1)
    }
}

impl Bingo {
    pub fn new(kind_specific_id: u8, kind: BingoKind, unique_id: Option<u8>) -> Self {
        Self {
            kind_specific_id,
            kind,
            unique_id,
        }
    }

    pub fn to_short_string(self) -> String {
        let prefix = match self.kind {
            BingoKind::Normal => "#",
            BingoKind::Extreme => "extreme #",
            BingoKind::Secret => "secret #",
        };
        format!("{prefix}{}", self.kind_specific_id + 1)
    }

    pub fn get_id(&self) -> u8 {
        self.unique_id.unwrap_or(self.kind_specific_id)
    }

    pub fn from_input(input: &str) -> Result<Self> {
        let error_help_message: String = format!("Failed to parse bingo identifier '{input}'");

        let mut input = input.to_lowercase();

        input = input.replace("bingo", "");
        input = input.replace('#', "");
        input = input.replace(' ', "");

        let (type_input, num_input) =
            if let Some(index) = input.find(|char: char| char.is_ascii_digit()) {
                let num_part = input.split_off(index);
                (input, num_part)
            } else {
                (input, String::new())
            };

        // no need to check for empty string, as `"normal".starts_with("")` is true
        let kind = if "normal".starts_with(&type_input) {
            BingoKind::Normal
        } else if "extreme".starts_with(&type_input) {
            BingoKind::Extreme
        } else if "secret".starts_with(&type_input) {
            BingoKind::Secret
        } else {
            bail!(UserError(anyhow!(error_help_message)));
        };

        let num: u8 = num_input
            .parse()
            .context(UserError(anyhow!(error_help_message)))?;

        if num == 0 {
            bail!(UserError(anyhow!("Bingo ID cannot be 0")));
        }

        Ok(Self {
            kind_specific_id: num - 1,
            kind,
            unique_id: None,
        })
    }
}

#[derive(Debug)]
pub enum SqlResponse {
    AffectedRows(usize),
    ReturnedRows(Vec<String>),
}

impl SqlResponse {
    pub fn to_formatted(&self) -> String {
        match self {
            SqlResponse::AffectedRows(rows) => format!("`{rows}` row(s) affected."),
            SqlResponse::ReturnedRows(rows) => format!("```\n{}\n```", rows.join("\n")),
        }
    }
}
