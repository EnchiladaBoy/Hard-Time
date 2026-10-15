//! Byte-compatible layouts from the pinned Data.bb / NativePreferences.bb.
//! Readers never write a player's files. Text bytes remain lossless, including
//! legacy Windows code-page names that are not UTF-8.
use std::{fmt, io};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeError {
    pub offset: usize,
    pub message: &'static str,
}
impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at byte {}", self.message, self.offset)
    }
}
impl std::error::Error for DecodeError {}
impl From<DecodeError> for io::Error {
    fn from(value: DecodeError) -> Self {
        Self::new(io::ErrorKind::InvalidData, value)
    }
}
type Result<T> = std::result::Result<T, DecodeError>;

pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    pub position: usize,
}
impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }
    pub fn error(&self, message: &'static str) -> DecodeError {
        DecodeError {
            offset: self.position,
            message,
        }
    }
    pub fn take(&mut self, count: usize) -> Result<&'a [u8]> {
        let end = self
            .position
            .checked_add(count)
            .ok_or_else(|| self.error("Length overflow"))?;
        let data = self
            .bytes
            .get(self.position..end)
            .ok_or_else(|| self.error("Truncated file"))?;
        self.position = end;
        Ok(data)
    }
    pub fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn i32(&mut self) -> Result<i32> {
        Ok(self.u32()? as i32)
    }
    pub fn f32(&mut self) -> Result<f32> {
        let value = f32::from_bits(self.u32()?);
        if !value.is_finite() {
            return Err(self.error("Non-finite floating-point value"));
        }
        Ok(value)
    }
    fn ints<const N: usize>(&mut self) -> Result<[i32; N]> {
        let mut values = [0; N];
        for value in &mut values {
            *value = self.i32()?;
        }
        Ok(values)
    }
    fn floats<const N: usize>(&mut self) -> Result<[f32; N]> {
        let mut values = [0.0; N];
        for value in &mut values {
            *value = self.f32()?;
        }
        Ok(values)
    }
    fn string(&mut self, limit: usize) -> Result<Vec<u8>> {
        let count = self.u32()? as usize;
        if count > limit {
            return Err(self.error("String exceeds format limit"));
        }
        Ok(self.take(count)?.to_vec())
    }
    pub fn finished(&self) -> Result<()> {
        if self.position != self.bytes.len() {
            return Err(self.error("Unexpected trailing data"));
        }
        Ok(())
    }
}
fn put_ints(bytes: &mut Vec<u8>, values: impl IntoIterator<Item = i32>) {
    for value in values {
        bytes.extend(value.to_le_bytes());
    }
}
fn put_floats(bytes: &mut Vec<u8>, values: impl IntoIterator<Item = f32>) {
    for value in values {
        bytes.extend(value.to_le_bytes());
    }
}
fn put_string(bytes: &mut Vec<u8>, value: &[u8]) {
    bytes.extend((value.len() as u32).to_le_bytes());
    bytes.extend(value);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub preferences: [i32; 6],
    pub keyboard: [i32; 4],
    pub controller: [i32; 4],
    pub slot_names: [Vec<u8>; 3],
}
impl Options {
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let mut r = Reader::new(bytes);
        let value = Self {
            preferences: r.ints()?,
            keyboard: r.ints()?,
            controller: r.ints()?,
            slot_names: [r.string(64)?, r.string(64)?, r.string(64)?],
        };
        r.finished()?;
        Ok(value)
    }
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        put_ints(&mut bytes, self.preferences);
        put_ints(&mut bytes, self.keyboard);
        put_ints(&mut bytes, self.controller);
        for name in &self.slot_names {
            put_string(&mut bytes, name);
        }
        bytes
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Progress {
    /// Count, controlled character, player handle, location, money.
    pub status: [i32; 5],
    /// Speed, seconds, minutes, hours.
    pub time: [i32; 4],
    pub mission: [i32; 5],
    pub handles: [i32; 10],
    pub phone_promo: i32,
    pub used_promos: [i32; 300],
    pub camera: [f32; 6],
    pub atmosphere: [f32; 12],
    pub trays: [i32; 50],
}
impl Progress {
    pub const BYTES: usize = 1572;
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let mut r = Reader::new(bytes);
        let value = Self {
            status: r.ints()?,
            time: r.ints()?,
            mission: r.ints()?,
            handles: r.ints()?,
            phone_promo: r.i32()?,
            used_promos: r.ints()?,
            camera: r.floats()?,
            atmosphere: r.floats()?,
            trays: r.ints()?,
        };
        r.finished()?;
        if !(1..=200).contains(&value.status[0])
            || !(1..=value.status[0]).contains(&value.status[1])
        {
            return Err(r.error("Invalid saved character count or player"));
        }
        Ok(value)
    }
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(Self::BYTES);
        put_ints(&mut bytes, self.status);
        put_ints(&mut bytes, self.time);
        put_ints(&mut bytes, self.mission);
        put_ints(&mut bytes, self.handles);
        put_ints(&mut bytes, [self.phone_promo]);
        put_ints(&mut bytes, self.used_promos);
        put_floats(&mut bytes, self.camera);
        put_floats(&mut bytes, self.atmosphere);
        put_ints(&mut bytes, self.trays);
        bytes
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Character {
    pub name: Vec<u8>,
    /// Appearance[9], scars[40], attributes[10], weapon history[30], status[7].
    pub properties: [i32; 96],
    pub position_and_yaw: [f32; 4],
    /// Gang[1], gang history[6], attacker/witness/promo/follow/bribe[5].
    pub relations_header: [i32; 12],
    pub relations: Vec<[i32; 3]>,
}
impl Character {
    pub fn decode(bytes: &[u8], character_count: usize) -> Result<Self> {
        let mut r = Reader::new(bytes);
        if !(1..=200).contains(&character_count) {
            return Err(r.error("Invalid character count"));
        }
        let mut value = Self {
            name: r.string(256)?,
            properties: r.ints()?,
            position_and_yaw: r.floats()?,
            relations_header: r.ints()?,
            relations: Vec::with_capacity(character_count),
        };
        for _ in 0..character_count {
            value.relations.push(r.ints()?);
        }
        r.finished()?;
        Ok(value)
    }
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        put_string(&mut bytes, &self.name);
        put_ints(&mut bytes, self.properties);
        put_floats(&mut bytes, self.position_and_yaw);
        put_ints(&mut bytes, self.relations_header);
        for relation in &self.relations {
            put_ints(&mut bytes, *relation);
        }
        bytes
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub kind_state_location: [i32; 3],
    pub position_and_yaw: [f32; 4],
    pub carrier_clip_ammo_scar: [i32; 4],
}
#[derive(Debug, Clone, PartialEq)]
pub struct Items {
    pub items: Vec<Item>,
    pub kits: [i32; 12],
}
impl Items {
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let mut r = Reader::new(bytes);
        let count = r.u32()? as usize;
        if count > 10_000 {
            return Err(r.error("Invalid saved item count"));
        }
        let mut items = Vec::with_capacity(count);
        for _ in 0..count {
            items.push(Item {
                kind_state_location: r.ints()?,
                position_and_yaw: r.floats()?,
                carrier_clip_ammo_scar: r.ints()?,
            });
        }
        let kits = r.ints()?;
        r.finished()?;
        Ok(Self { items, kits })
    }
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        put_ints(&mut bytes, [self.items.len() as i32]);
        for item in &self.items {
            put_ints(&mut bytes, item.kind_state_location);
            put_floats(&mut bytes, item.position_and_yaw);
            put_ints(&mut bytes, item.carrier_clip_ammo_scar);
        }
        put_ints(&mut bytes, self.kits);
        bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeOptions {
    pub values: [i32; 18],
}
impl NativeOptions {
    const MAGIC: i32 = 1213484356;
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let mut r = Reader::new(bytes);
        if r.i32()? != Self::MAGIC || r.i32()? != 1 {
            return Err(r.error("Unrecognized modern settings version"));
        }
        let values = r.ints::<18>()?;
        let checksum = values.iter().fold(Self::MAGIC, |sum, value| sum ^ value);
        if r.i32()? != checksum {
            return Err(r.error("Modern settings checksum mismatch"));
        }
        r.finished()?;
        let ranges = [
            1..=9,
            0..=1,
            75..=125,
            25..=300,
            50..=150,
            0..=1,
            0..=100,
            0..=100,
            0..=100,
        ];
        if ranges
            .iter()
            .zip(values)
            .any(|(range, value)| !range.contains(&value))
        {
            return Err(r.error("Modern settings value out of range"));
        }
        for key in &values[9..] {
            if !(2..=211).contains(key) || [28, 56, 184, 87, 88].contains(key) {
                return Err(r.error("Invalid reserved key binding"));
            }
        }
        Ok(Self { values })
    }
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        put_ints(&mut bytes, [Self::MAGIC, 1]);
        put_ints(&mut bytes, self.values);
        put_ints(
            &mut bytes,
            [self
                .values
                .iter()
                .fold(Self::MAGIC, |sum, value| sum ^ value)],
        );
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_names_keep_non_utf8_bytes_and_empty_slots() {
        let mut bytes = Vec::new();
        put_ints(&mut bytes, [2, 80, 1, 1, 2, 3, 30, 44, 31, 45, 1, 2, 3, 4]);
        bytes.extend([4, 0, 0, 0, b'J', 0xf6, b'r', b'g', 0, 0, 0, 0, 0, 0, 0, 0]);
        let value = Options::decode(&bytes).unwrap();
        assert_eq!(value.slot_names[0], b"J\xf6rg");
        assert_eq!(value.encode(), bytes);
        bytes[56..60].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(Options::decode(&bytes).is_err());
    }
    #[test]
    fn progress_matches_original_field_offsets() {
        let mut bytes = vec![0; Progress::BYTES];
        bytes[0..4].copy_from_slice(&80_i32.to_le_bytes());
        bytes[4..8].copy_from_slice(&7_i32.to_le_bytes());
        bytes[16..20].copy_from_slice(&345_i32.to_le_bytes());
        bytes[1300..1304].copy_from_slice(&12.5_f32.to_le_bytes());
        bytes[1568..1572].copy_from_slice(&3_i32.to_le_bytes());
        let value = Progress::decode(&bytes).unwrap();
        assert_eq!(value.status[4], 345);
        assert_eq!(value.camera[0], 12.5);
        assert_eq!(value.trays[49], 3);
        assert_eq!(value.encode(), bytes);
        for length in [0, 4, 1301, 1571] {
            assert!(Progress::decode(&bytes[..length]).is_err());
        }
        bytes[1300..1304].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(Progress::decode(&bytes).is_err());
    }
    #[test]
    fn character_relationship_count_is_part_of_the_save_format() {
        let mut bytes = vec![0; 4 + 96 * 4 + 4 * 4 + 12 * 4 + 3 * 4 * 80];
        bytes[4 + 96 * 4..4 + 97 * 4].copy_from_slice(&(-37.5_f32).to_le_bytes());
        let value = Character::decode(&bytes, 80).unwrap();
        assert_eq!(value.position_and_yaw[0], -37.5);
        assert_eq!(value.relations.len(), 80);
        assert_eq!(value.encode(), bytes);
        assert!(Character::decode(&bytes, 79).is_err());
        assert!(Character::decode(&bytes, usize::MAX).is_err());
    }
    #[test]
    fn weapon_positions_and_kit_records_roundtrip() {
        let mut bytes = Vec::new();
        put_ints(&mut bytes, [1, 4, 1, 9]);
        put_floats(&mut bytes, [1.0, 2.0, -3.0, 90.0]);
        put_ints(&mut bytes, [0, 5, 6, 7]);
        put_ints(&mut bytes, 0..12);
        let items = Items::decode(&bytes).unwrap();
        assert_eq!(items.items[0].position_and_yaw, [1.0, 2.0, -3.0, 90.0]);
        assert_eq!(items.kits[11], 11);
        assert_eq!(items.encode(), bytes);
        bytes[0..4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(Items::decode(&bytes).is_err());
    }
    #[test]
    fn modern_settings_detect_corruption_and_reserved_keys() {
        let mut options = NativeOptions {
            values: [
                2, 0, 100, 100, 100, 1, 100, 100, 100, 200, 208, 203, 205, 30, 44, 31, 45, 25,
            ],
        };
        let bytes = options.encode();
        assert_eq!(bytes.len(), 84);
        assert_eq!(NativeOptions::decode(&bytes).unwrap(), options);
        for index in 0..bytes.len() {
            let mut corrupted = bytes.clone();
            corrupted[index] ^= 1;
            assert!(NativeOptions::decode(&corrupted).is_err(), "byte {index}");
        }
        options.values[9] = 88;
        assert!(NativeOptions::decode(&options.encode()).is_err());
    }
}
