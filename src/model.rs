//! 處理過的緊湊資料結構。UI 執行緒只碰這些，不碰 JSON。

use serde::{Deserialize, Serialize};

/// 篩選條件：模式、牌位、版本。
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Filter {
    pub game_type: u16,
    pub rank_group: u8,
    pub patch: u32,
}

impl Default for Filter {
    fn default() -> Self {
        Self { game_type: 1100, rank_group: 1, patch: crate::net::KNOWN_PATCHES[0] }
    }
}

pub const GAME_TYPES: [(u16, &str); 2] = [(1100, "積分"), (1160, "雙人作戰")];
pub const RANK_GROUPS: [(u8, &str); 5] =
    [(4, "宗師以上"), (0, "大師以上"), (1, "鑽石以上"), (2, "翡翠以上"), (3, "白金以上")];

pub fn patch_label(p: u32) -> String {
    let major = p / 1000;
    let minor = (p % 1000) / 10;
    let sub = p % 10;
    if sub == 0 { format!("{major}.{minor}") } else { format!("{major}.{minor}.{sub}") }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UnitRow {
    pub id: String,
    pub count: u32,
    pub place: f32,
    pub top4: f32,
    pub won: f32,
    pub top_items: Vec<String>,
    pub star_count: u32,
    pub star_place: f32,
    pub star_top4: f32,
    pub star_won: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ItemRow {
    pub id: String,
    pub count: u32,
    pub place: f32,
    pub top4: f32,
    pub won: f32,
    /// (英雄 id, 名次差)；越負代表給這隻英雄越好
    pub top_users: Vec<(String, f32)>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TraitRow {
    pub id: String,
    /// 1 起算的階段索引（對應 CDragon effects[tier-1]）
    pub tier: u8,
    pub count: u32,
    pub place: f32,
    pub top4: f32,
    pub won: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct StatsData {
    pub total_entries: u64,
    pub last_updated: u64,
    pub units: Vec<UnitRow>,
    pub items: Vec<ItemRow>,
    pub traits: Vec<TraitRow>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CompRow {
    pub count: u32,
    pub place: f32,
    /// 百分比
    pub top4: f32,
    /// 百分比
    pub win: f32,
    pub lp_delta: f32,
    pub carries: Vec<String>,
    /// 常駐英雄（出現率 >= 50%），依出現率排序
    pub core_units: Vec<String>,
    /// (羈絆 id, 階段)，依出現率排序
    pub traits: Vec<(String, u8)>,
    pub detail: CompDetail,
}

/// 點進陣容後顯示的詳細資料。
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct CompDetail {
    /// 出場率：平均每場對局（8 人）有幾位玩家使用
    pub play_rate: f32,
    /// 核心英雄（出現率 >= 50%）：(id, 最常見星級, 出現率)
    pub core: Vec<(String, u8, f32)>,
    /// 彈性英雄（出現率 25%～50%）
    pub flex: Vec<(String, u8, f32)>,
    /// 每隻英雄最好的裝備：(英雄, [裝備])
    pub unit_items: Vec<(String, Vec<String>)>,
    /// 裝備排行（名次差越負越好）：(裝備, [適合的英雄], 名次差)
    pub item_tiers: Vec<(String, Vec<String>, f32)>,
    /// 主力英雄權重
    pub carries: Vec<(String, f32)>,
    /// 等級分布：(標籤, 佔比 0~1, 平均名次)
    pub levels: Vec<(String, f32, f32)>,
    /// 1~8 名的佔比 0~1
    pub placements: Vec<f32>,
    /// 勝利條件：(英雄, 星級, 出現率 0~1, 平均名次)
    pub win_cons: Vec<(String, u8, f32, f32)>,
    /// 羈絆：(id, 階段, 出現率 0~1, 平均名次)
    pub traits: Vec<(String, u8, f32, f32)>,
    pub variants: Vec<Variant>,
    pub lvl9: Vec<Variant>,
    /// (伺服器, 玩家名稱)
    pub top_players: Vec<(String, String)>,
    /// 遊戲內陣容規劃器可匯入的代碼
    pub code: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Variant {
    pub units: Vec<String>,
    pub spat: Vec<String>,
    pub count: u32,
    pub place: f32,
    pub top4: f32,
    pub win: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct CompsData {
    pub total: u32,
    pub comps: Vec<CompRow>,
}

/// 英雄／裝備／羈絆的中文名稱與圖示。
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct StaticEntry {
    pub name: String,
    pub cost: u8,
    pub icon: String,
    /// 羈絆各階段所需人數；非羈絆為空
    pub tiers: Vec<u8>,
    /// 英雄的羈絆（中文名稱）
    pub traits: Vec<String>,
    /// 裝備的合成材料（id）
    pub parts: Vec<String>,
    /// 英文名稱（搜尋用）
    pub en: String,
}

/// 裝備類別（從 id 判斷）
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ItemCat {
    Normal,
    Radiant,
    Artifact,
    Emblem,
}

pub const ITEM_CATS: [(ItemCat, &str); 4] =
    [(ItemCat::Normal, "一般"), (ItemCat::Radiant, "聖光"), (ItemCat::Artifact, "神化"), (ItemCat::Emblem, "徽章")];

pub fn item_cat(id: &str) -> ItemCat {
    if id.contains("Radiant") {
        ItemCat::Radiant
    } else if id.contains("Artifact") {
        ItemCat::Artifact
    } else if id.contains("Emblem") {
        ItemCat::Emblem
    } else {
        ItemCat::Normal
    }
}

// ---------- 英雄／裝備詳細資料 ----------

/// 基本表現數據
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default)]
pub struct Perf {
    pub count: u32,
    pub place: f32,
    pub top4: f32,
    pub won: f32,
}

/// 搭配對象（英雄／羈絆／裝備）與名次差（越負越好）
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Rel {
    pub id: String,
    /// 羈絆階段；其他為 0
    pub tier: u8,
    pub perf: Perf,
    pub delta: f32,
}

/// 出裝組合
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Build {
    pub items: Vec<String>,
    pub perf: Perf,
    pub delta: f32,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Hash, Debug)]
pub struct EntityKey {
    pub is_item: bool,
    pub filter: Filter,
    pub id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct EntityDetail {
    pub base: Perf,
    /// 平均每場對局出現幾次
    pub rate: f32,
    /// 1~8 名佔比
    pub placements: Vec<f32>,
    /// 星級 → 表現
    pub stars: Vec<(u8, Perf)>,
    /// 裝備數量 → 表現
    pub item_counts: Vec<(u8, Perf)>,
    pub builds1: Vec<Build>,
    pub builds2: Vec<Build>,
    pub builds3: Vec<Build>,
    pub units: Vec<Rel>,
    pub traits: Vec<Rel>,
    pub items: Vec<Rel>,
    pub regions: Vec<(String, Perf)>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Statics {
    pub fetched_at: u64,
    pub map: std::collections::HashMap<String, StaticEntry>,
}

impl Statics {
    pub fn name<'a>(&'a self, id: &'a str) -> &'a str {
        match self.map.get(id) {
            Some(e) if !e.name.is_empty() => &e.name,
            _ => pretty_id(id),
        }
    }
    pub fn icon(&self, id: &str) -> Option<&str> {
        self.map.get(id).map(|e| e.icon.as_str()).filter(|s| !s.is_empty())
    }
    pub fn cost(&self, id: &str) -> u8 {
        self.map.get(id).map(|e| e.cost).unwrap_or(0)
    }
    /// 英文名稱；沒有時用 id 去掉前綴
    pub fn en<'a>(&'a self, id: &'a str) -> &'a str {
        match self.map.get(id) {
            Some(e) if !e.en.is_empty() => &e.en,
            _ => pretty_id(id),
        }
    }
    pub fn entry(&self, id: &str) -> Option<&StaticEntry> {
        self.map.get(id)
    }
    /// 羈絆階段 → 人數（找不到時回傳 None）
    pub fn trait_units(&self, id: &str, tier: u8) -> Option<u8> {
        let e = self.map.get(id)?;
        e.tiers.get((tier as usize).checked_sub(1)?).copied().filter(|&n| n > 0)
    }
}

/// 沒有中文名稱時，把 `DA_18_Teemo` 簡化成 `Teemo`。
pub fn pretty_id(id: &str) -> &str {
    id.rsplit('_').next().unwrap_or(id)
}

pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
