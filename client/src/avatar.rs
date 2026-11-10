//! Blob character definition. Encoded for mDNS; drawn by the GUI.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyShape {
    Round,
    Squish,
    Tall,
    Bean,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Eyes {
    Dots,
    Happy,
    Wide,
    Sleepy,
    Wink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mouth {
    Smile,
    Grin,
    Oh,
    Flat,
    Smirk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Accessory {
    None,
    Glasses,
    Cap,
    Sprout,
    Bow,
}

/// Soft pastel palette indexes shared by the painter and the creator.
pub const COLOR_COUNT: u8 = 8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Avatar {
    pub shape: BodyShape,
    /// Index into the shared color palette (`0..COLOR_COUNT`).
    pub color: u8,
    pub eyes: Eyes,
    pub mouth: Mouth,
    pub accessory: Accessory,
}

impl Default for Avatar {
    fn default() -> Self {
        Self {
            shape: BodyShape::Round,
            color: 0,
            eyes: Eyes::Dots,
            mouth: Mouth::Smile,
            accessory: Accessory::None,
        }
    }
}

impl Avatar {
    pub fn random() -> Self {
        let n = mix(random_seed());
        Self::from_seed(n)
    }

    /// Stable look derived from a display name when a peer has no avatar TXT.
    pub fn from_name(name: &str) -> Self {
        let mut hash = 2166136261u32;
        for byte in name.bytes() {
            hash ^= u32::from(byte);
            hash = hash.wrapping_mul(16777619);
        }
        Self::from_seed(hash)
    }

    fn from_seed(seed: u32) -> Self {
        let shapes = [
            BodyShape::Round,
            BodyShape::Squish,
            BodyShape::Tall,
            BodyShape::Bean,
        ];
        let eyes = [
            Eyes::Dots,
            Eyes::Happy,
            Eyes::Wide,
            Eyes::Sleepy,
            Eyes::Wink,
        ];
        let mouths = [
            Mouth::Smile,
            Mouth::Grin,
            Mouth::Oh,
            Mouth::Flat,
            Mouth::Smirk,
        ];
        let accessories = [
            Accessory::None,
            Accessory::Glasses,
            Accessory::Cap,
            Accessory::Sprout,
            Accessory::Bow,
            Accessory::None,
        ];
        Self {
            shape: shapes[(seed as usize) % shapes.len()],
            color: (seed.wrapping_mul(7) % u32::from(COLOR_COUNT)) as u8,
            eyes: eyes[((seed >> 8) as usize) % eyes.len()],
            mouth: mouths[((seed >> 16) as usize) % mouths.len()],
            accessory: accessories[((seed >> 20) as usize) % accessories.len()],
        }
    }

    /// Compact mDNS-safe encoding: `v1:shape:color:eyes:mouth:acc`.
    pub fn encode(&self) -> String {
        format!(
            "v1:{}:{}:{}:{}:{}",
            shape_id(self.shape),
            self.color.min(COLOR_COUNT - 1),
            eyes_id(self.eyes),
            mouth_id(self.mouth),
            accessory_id(self.accessory),
        )
    }

    pub fn decode(raw: &str) -> Option<Self> {
        let raw = raw.trim();
        if raw.is_empty() {
            return None;
        }
        let mut parts = raw.split(':');
        if parts.next()? != "v1" {
            return None;
        }
        let shape = shape_from(parts.next()?.parse().ok()?)?;
        let color = parts.next()?.parse::<u8>().ok()?.min(COLOR_COUNT - 1);
        let eyes = eyes_from(parts.next()?.parse().ok()?)?;
        let mouth = mouth_from(parts.next()?.parse().ok()?)?;
        let accessory = accessory_from(parts.next()?.parse().ok()?)?;
        if parts.next().is_some() {
            return None;
        }
        Some(Self {
            shape,
            color,
            eyes,
            mouth,
            accessory,
        })
    }
}

fn shape_id(shape: BodyShape) -> u8 {
    match shape {
        BodyShape::Round => 0,
        BodyShape::Squish => 1,
        BodyShape::Tall => 2,
        BodyShape::Bean => 3,
    }
}

fn shape_from(id: u8) -> Option<BodyShape> {
    Some(match id {
        0 => BodyShape::Round,
        1 => BodyShape::Squish,
        2 => BodyShape::Tall,
        3 => BodyShape::Bean,
        _ => return None,
    })
}

fn eyes_id(eyes: Eyes) -> u8 {
    match eyes {
        Eyes::Dots => 0,
        Eyes::Happy => 1,
        Eyes::Wide => 2,
        Eyes::Sleepy => 3,
        Eyes::Wink => 4,
    }
}

fn eyes_from(id: u8) -> Option<Eyes> {
    Some(match id {
        0 => Eyes::Dots,
        1 => Eyes::Happy,
        2 => Eyes::Wide,
        3 => Eyes::Sleepy,
        4 => Eyes::Wink,
        _ => return None,
    })
}

fn mouth_id(mouth: Mouth) -> u8 {
    match mouth {
        Mouth::Smile => 0,
        Mouth::Grin => 1,
        Mouth::Oh => 2,
        Mouth::Flat => 3,
        Mouth::Smirk => 4,
    }
}

fn mouth_from(id: u8) -> Option<Mouth> {
    Some(match id {
        0 => Mouth::Smile,
        1 => Mouth::Grin,
        2 => Mouth::Oh,
        3 => Mouth::Flat,
        4 => Mouth::Smirk,
        _ => return None,
    })
}

fn accessory_id(accessory: Accessory) -> u8 {
    match accessory {
        Accessory::None => 0,
        Accessory::Glasses => 1,
        Accessory::Cap => 2,
        Accessory::Sprout => 3,
        Accessory::Bow => 4,
    }
}

fn accessory_from(id: u8) -> Option<Accessory> {
    Some(match id {
        0 => Accessory::None,
        1 => Accessory::Glasses,
        2 => Accessory::Cap,
        3 => Accessory::Sprout,
        4 => Accessory::Bow,
        _ => return None,
    })
}

fn random_seed() -> u32 {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    use std::time::{SystemTime, UNIX_EPOCH};

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0);
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u64(nanos);
    hasher.write(b"avatar");
    hasher.finish() as u32
}

fn mix(mut x: u32) -> u32 {
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let avatar = Avatar {
            shape: BodyShape::Bean,
            color: 5,
            eyes: Eyes::Wink,
            mouth: Mouth::Grin,
            accessory: Accessory::Sprout,
        };
        let again = Avatar::decode(&avatar.encode()).unwrap();
        assert_eq!(avatar, again);
    }

    #[test]
    fn from_name_is_stable() {
        assert_eq!(Avatar::from_name("Ada"), Avatar::from_name("Ada"));
    }
}
