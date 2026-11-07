//! Break kinds: every timing, color and tip lives here as data. New kind = new row. 💅

/// Set of kinds: bit `i` means `KINDS[i]`.
pub type Mask = u8;

/// One break kind. `natural`: idling this long counts as the break (water: no, idling never hydrates).
pub struct Kind {
    pub name: &'static str,
    pub key: &'static str,
    pub every_ms: u32,
    pub length_ms: u32,
    pub natural: bool,
    pub accent: u32,
    pub tips: &'static [&'static str],
}

const SEC: u32 = 1_000;
const MIN: u32 = 60 * SEC;

/// GDI COLORREF (0x00BBGGRR) from web hex 0xRRGGBB.
pub const fn rgb(hex: u32) -> u32 {
    (hex >> 16) & 0xFF | hex & 0xFF00 | (hex & 0xFF) << 16
}

pub const KINDS: [Kind; 4] = [
    Kind {
        name: "Micro-break",
        key: "micro",
        every_ms: 10 * MIN,
        length_ms: 20 * SEC,
        natural: true,
        accent: rgb(0x2DD4BF),
        tips: &[
            "Roll your shoulders back slowly, five times.",
            "Stretch both arms overhead and take a deep breath.",
            "Turn your head slowly left, then right. Hold each side.",
            "Open and close your hands ten times, then shake them loose.",
            "Stand up and gently stretch your back.",
            "Drop your shoulders, unclench your jaw, breathe out slowly.",
        ],
    },
    Kind {
        name: "Eye break",
        key: "eye",
        every_ms: 20 * MIN,
        length_ms: 20 * SEC,
        natural: true,
        accent: rgb(0xFBBF24),
        tips: &[
            "Look at something 6 m (20 ft) away until the timer ends.",
            "Rest your eyes on something 6 m (20 ft) away. Blink slowly.",
        ],
    },
    Kind {
        name: "Rest break",
        key: "rest",
        every_ms: 60 * MIN,
        length_ms: 5 * MIN,
        natural: true,
        accent: rgb(0xA78BFA),
        tips: &[
            "Stand up and walk around for a few minutes.",
            "Step away from the screen. Look out a window, breathe.",
            "Walk a little, stretch your legs and loosen your hips.",
            "Do a few slow squats or calf raises, then walk around.",
        ],
    },
    Kind {
        name: "Water break",
        key: "water",
        every_ms: 45 * MIN,
        length_ms: 15 * SEC,
        natural: false,
        accent: rgb(0x60A5FA),
        tips: &["Drink a glass of water.", "Time for a few sips of water."],
    },
];

/// (index, kind) for each kind in `mask`, in table order.
pub fn in_mask(mask: Mask) -> impl Iterator<Item = (usize, &'static Kind)> {
    KINDS
        .iter()
        .enumerate()
        .filter(move |(i, _)| mask >> i & 1 == 1)
}
