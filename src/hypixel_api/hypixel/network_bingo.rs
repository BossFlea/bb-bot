use std::collections::HashMap;

use serde_json::Value;

use crate::role::types::NetworkBingo;

pub fn network_bingo_completions(seasonal: &Value) -> Vec<NetworkBingo> {
    let mut bingos = Vec::new();

    let checks = [
        (
            ExpectedGoals::new(ANNIVERSARY_2023)
                .is_fully_completed(&seasonal["anniversary"][" 2023"]["bingo"]), // HYPIXEL???
            NetworkBingo::Anniversary2023,
        ),
        (
            has_all_difficulties(&seasonal["halloween"]["2023"]["bingo"]),
            NetworkBingo::Halloween2023,
        ),
        (
            has_all_difficulties(&seasonal["christmas"]["2023"]["bingo"]),
            NetworkBingo::Christmas2023,
        ),
        (
            has_all_difficulties(&seasonal["easter"]["2024"]["bingo"]),
            NetworkBingo::Easter2024,
        ),
        (
            has_all_difficulties(&seasonal["summer"]["2024"]["bingo"]),
            NetworkBingo::Summer2024,
        ),
        (
            has_all_difficulties(&seasonal["halloween"]["2024"]["bingo"]),
            NetworkBingo::Halloween2024,
        ),
        (
            has_any_difficulty_pair(&seasonal["easter"]["2025"]["bingo"]),
            NetworkBingo::Anniversary2025,
        ),
    ];

    checks.into_iter().for_each(|(completed, bingo)| {
        if completed {
            bingos.push(bingo);
        }
    });

    bingos
}

fn has_all_difficulties(bingo_json: &Value) -> bool {
    has_blackouts_for(
        bingo_json,
        &[Difficulty::Easy, Difficulty::Medium, Difficulty::Hard],
        None,
    )
}
fn has_any_difficulty_pair(bingo_json: &Value) -> bool {
    has_blackouts_for(
        bingo_json,
        &[Difficulty::Easy, Difficulty::Hard],
        Some(&[CardType::Casual, CardType::PvP, CardType::Classic]),
    )
}

fn has_blackouts_for(bingo_json: &Value, diffs: &[Difficulty], cards: Option<&[CardType]>) -> bool {
    diffs.iter().all(|diff| match cards {
        Some(cards) => cards
            .iter()
            .any(|card| has_type_difficulty_blackout(bingo_json, *diff, *card)),
        None => has_difficulty_blackout(bingo_json, *diff),
    })
}

fn has_difficulty_blackout(bingo_json: &Value, diff: Difficulty) -> bool {
    bingo_json[diff.as_str()]["rewards"]
        .as_array()
        .map(|r| r.iter().any(|v| v.as_str() == Some("black_out")))
        .unwrap_or(false)
}
fn has_type_difficulty_blackout(bingo_json: &Value, diff: Difficulty, card: CardType) -> bool {
    bingo_json[format!("{}_{}", card.as_str(), diff.as_str())]["rewards"]
        .as_array()
        .map(|r| r.iter().any(|v| v.as_str() == Some("black_out")))
        .unwrap_or(false)
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
enum Difficulty {
    Easy,
    Medium,
    Hard,
}

impl Difficulty {
    fn as_str(&self) -> &str {
        match self {
            Difficulty::Easy => "easy",
            Difficulty::Medium => "medium",
            Difficulty::Hard => "hard",
        }
    }
}

#[derive(Copy, Clone)]
enum CardType {
    Casual,
    PvP,
    Classic,
}

impl CardType {
    fn as_str(&self) -> &str {
        match self {
            CardType::Casual => "casual",
            CardType::PvP => "pvp",
            CardType::Classic => "classic",
        }
    }
}

/// Goal completion detection for bingos where there aren't any `rewards` on the API
struct ExpectedGoals {
    objectives: HashMap<Difficulty, HashMap<String, i64>>,
}

impl ExpectedGoals {
    fn new(goals: &[&[(&str, i64)]; 3]) -> Self {
        let mut objectives = HashMap::new();

        objectives.insert(
            Difficulty::Easy,
            goals[0].iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        );
        objectives.insert(
            Difficulty::Medium,
            goals[1].iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        );
        objectives.insert(
            Difficulty::Hard,
            goals[2].iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        );

        Self { objectives }
    }

    fn expected(&self, diff: Difficulty) -> &HashMap<String, i64> {
        &self.objectives[&diff]
    }

    fn is_completed(&self, bingo_json: &Value, diff: Difficulty) -> bool {
        let Some(obj) = bingo_json
            .get(diff.as_str())
            .and_then(|d| d.get("objectives"))
        else {
            return false;
        };

        let objectives: HashMap<String, i64> =
            serde_json::from_value(obj.to_owned()).unwrap_or_default();

        objectives == *self.expected(diff)
    }

    fn is_fully_completed(&self, bingo_json: &Value) -> bool {
        self.is_completed(bingo_json, Difficulty::Easy)
            && self.is_completed(bingo_json, Difficulty::Medium)
            && self.is_completed(bingo_json, Difficulty::Hard)
    }
}

const ANNIVERSARY_2023: &[&[(&str, i64)]; 3] = &[
    ANNIVERSARY_2023_EASY,
    ANNIVERSARY_2023_MEDIUM,
    ANNIVERSARY_2023_HARD,
];

const ANNIVERSARY_2023_EASY: &[(&str, i64)] = &[
    ("Blitzchests", 5),
    ("Cvcthrowprojectile", 1),
    ("Pitkill", 1),
    ("Arenaultimate", 1),
    ("Wizardscapture", 1),
    ("Arcadekillcreeper", 2),
    ("Smashthrowoff", 1),
    ("Maincatchfish", 25),
    ("Vampzvampirekill", 1),
    ("Tntrunsurviveminute", 1),
    ("Wallswoodpickaxe", 1),
    ("Arcadezombiesdoor", 1),
    ("Tkrcollectbox", 1),
    ("Wwplacewool", 1),
    ("Bbguess", 1),
    ("Arcadeblockingdeadkills", 20),
    ("Skywarsvoidkill", 1),
    ("Megawallsdefense", 1),
    ("Quakedash", 1),
    ("Bedwarsdiamond", 1),
    ("Pixelpartysurvive", 3),
    ("Tnttagplayer", 1),
    ("Pbpowerup", 1),
    ("Murderbowgold", 1),
    ("Arcadehiderdamage", 1),
];
const ANNIVERSARY_2023_MEDIUM: &[(&str, i64)] = &[
    ("Arcadetwowithers", 2),
    ("Bedwarsemerald", 1),
    ("Blitzshutdown", 1),
    ("Pvprunkill", 1),
    ("Vampzsurvivorkill", 1),
    ("Skywarsdiamondarmor", 1),
    ("Tkrbanana", 1),
    ("Arcadedragonkill", 1),
    ("Wwflawless", 1),
    ("Arcadesupplychests", 6),
    ("Warlordsdamageflag", 1),
    ("Arcadetop3round", 1),
    ("Pbtntrain", 1),
    ("Bowspleefsurvivetwo", 1),
    ("Maincatchtreasure", 15),
    ("Arcademegapunch", 1),
    ("Arenabuffs", 2),
    ("Quakeheadshot", 1),
    ("Megawallsfinaltwo", 1),
    ("Cvcclosecall", 1),
    ("Pitpickupgold", 10),
    ("Arcadehitwperfect", 1),
    ("Murderkillmurderer", 1),
    ("Smashnemesis", 1),
    ("Duelsparkour3rd", 1),
];
const ANNIVERSARY_2023_HARD: &[(&str, i64)] = &[
    ("Murderstreak", 10),
    ("Blitzstar", 1),
    ("Smashtwolives", 1),
    ("Arcadehiderpro", 3),
    ("Bedwarsemeraldhoarder", 10),
    ("Bbfastguess", 1),
    ("Arcadewoolcarrier", 1),
    ("Quake5streak", 1),
    ("Wwnoenemywool", 1),
    ("Arcadeenderspleef", 1),
    ("Cvcallaround", 1),
    ("Wallsdiamond", 1),
    ("Skywarschallenge", 1),
    ("Bedwarsflawless", 1),
    ("Arcadedontmove", 1),
    ("Arcadehypixelsays", 1),
    ("Arcadezombies25", 1),
    ("Vampzsurvive", 1),
    ("Arcadebountyhunters", 1),
    ("Warlordscapture", 1),
    ("Arcadehelp", 1),
    ("Wizardslandslide", 1),
    ("Bbtop3", 1),
    ("Megawallsfinal", 1),
    ("Pbnuke", 1),
];
