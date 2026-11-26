//! 把三種原始 JSON 轉成 model.rs 的緊湊結構。只在背景執行緒呼叫。

use crate::model::*;
use serde::Deserialize;
use std::collections::HashMap;

// ---------- stats2/general ----------

#[derive(Deserialize)]
struct RawStats {
    #[serde(default, rename = "totalEntries")]
    total_entries: u64,
    #[serde(default, rename = "lastUpdated")]
    last_updated: u64,
    #[serde(default)]
    units: HashMap<String, RawUnit>,
    #[serde(default)]
    traits: HashMap<String, RawPerf>,
    #[serde(default)]
    items: Vec<RawItem>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct RawUnit {
    count: u32,
    place: f32,
    top4: f32,
    won: f32,
    top_items: Vec<String>,
    star_count: u32,
    star_place: f32,
    star_top4: f32,
    star_won: f32,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawPerf {
    count: u32,
    place: f32,
    top4: f32,
    won: f32,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct RawItem {
    item_id: String,
    count: u32,
    place: f32,
    top4: f32,
    won: f32,
    top_users: Vec<(String, f32)>,
}

pub fn parse_stats(bytes: &[u8]) -> Result<StatsData, String> {
    let raw: RawStats = serde_json::from_slice(bytes).map_err(|e| format!("統計資料格式錯誤：{e}"))?;
    let mut units: Vec<UnitRow> = raw
        .units
        .into_iter()
        .map(|(id, u)| UnitRow {
            id,
            count: u.count,
            place: u.place,
            top4: u.top4,
            won: u.won,
            top_items: u.top_items,
            star_count: u.star_count,
            star_place: u.star_place,
            star_top4: u.star_top4,
            star_won: u.star_won,
        })
        .collect();
    units.sort_unstable_by(|a, b| a.place.total_cmp(&b.place));

    let mut items: Vec<ItemRow> = raw
        .items
        .into_iter()
        .map(|i| ItemRow {
            id: i.item_id,
            count: i.count,
            place: i.place,
            top4: i.top4,
            won: i.won,
            top_users: i.top_users,
        })
        .collect();
    items.sort_unstable_by(|a, b| a.place.total_cmp(&b.place));

    let mut traits: Vec<TraitRow> = raw
        .traits
        .into_iter()
        .map(|(key, t)| {
            let (id, tier) = match key.rsplit_once("__") {
                Some((id, n)) => (id.to_string(), n.parse().unwrap_or(1)),
                None => (key, 1),
            };
            TraitRow { id, tier, count: t.count, place: t.place, top4: t.top4, won: t.won }
        })
        .collect();
    traits.sort_unstable_by(|a, b| a.place.total_cmp(&b.place));

    Ok(StatsData {
        total_entries: raw.total_entries,
        last_updated: raw.last_updated,
        units,
        items,
        traits,
    })
}

// ---------- team-compositions ----------

#[derive(Deserialize)]
struct RawComps {
    #[serde(default)]
    groups: Vec<RawGroup>,
    #[serde(default)]
    count: u32,
}

#[derive(Deserialize)]
struct RawGroup {
    full: RawComp,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct RawComp {
    count: u32,
    place: f32,
    top4: u32,
    win: u32,
    lp_delta: f32,
    carry_units: Vec<(String, f64)>,
    traits: Vec<(String, f64, f64, f64)>,
    units: Vec<(String, f64, f64, f64)>,
    item_deltas: Vec<(String, String, f64)>,
    placement_distribution: Vec<f64>,
    /// [等級, 場數, 名次總和]
    levels: Vec<(f64, f64, f64)>,
    comps: Vec<RawVariant>,
    lvl9_comps: Vec<RawVariant>,
    top_players: Vec<serde_json::Value>,
    code: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct RawVariant {
    units: Vec<String>,
    spat_items: Vec<String>,
    count: u32,
    place: f32,
    top4: u32,
    win: u32,
}

fn variant(v: RawVariant) -> Variant {
    let n = v.count.max(1) as f32;
    Variant {
        units: v.units,
        spat: v.spat_items,
        count: v.count,
        place: v.place,
        top4: v.top4 as f32 * 100.0 / n,
        win: v.win as f32 * 100.0 / n,
    }
}

pub fn parse_comps(bytes: &[u8]) -> Result<CompsData, String> {
    let raw: RawComps = serde_json::from_slice(bytes).map_err(|e| format!("陣容資料格式錯誤：{e}"))?;
    let total = raw.count.max(1) as f32;
    let mut comps: Vec<CompRow> = raw.groups.into_iter().map(|g| comp_row(g.full, total)).collect();
    comps.sort_unstable_by(|a, b| a.place.total_cmp(&b.place));
    Ok(CompsData { total: raw.count, comps })
}

fn comp_row(c: RawComp, total: f32) -> CompRow {
    let n = c.count.max(1) as f64;

    // 英雄出現率：同一隻英雄不同星級加總；星級取最常見的
    let mut agg: Vec<(String, f64, u8, f64)> = Vec::new(); // (id, 總場數, 最常見星級, 該星級場數)
    for (id, star, cnt, _) in &c.units {
        match agg.iter_mut().find(|a| &a.0 == id) {
            Some(a) => {
                a.1 += cnt;
                if *cnt > a.3 {
                    a.2 = *star as u8;
                    a.3 = *cnt;
                }
            }
            None => agg.push((id.clone(), *cnt, *star as u8, *cnt)),
        }
    }
    agg.sort_unstable_by(|a, b| b.1.total_cmp(&a.1));
    let share = |x: f64| (x / n).min(1.0) as f32;
    let core: Vec<(String, u8, f32)> =
        agg.iter().filter(|a| a.1 / n >= 0.5).map(|a| (a.0.clone(), a.2, share(a.1))).collect();
    let flex: Vec<(String, u8, f32)> = agg
        .iter()
        .filter(|a| a.1 / n >= 0.25 && a.1 / n < 0.5)
        .map(|a| (a.0.clone(), a.2, share(a.1)))
        .collect();

    let carries_w: Vec<(String, f32)> = c.carry_units.iter().map(|(id, w)| (id.clone(), *w as f32)).collect();
    let carries: Vec<String> = c.carry_units.into_iter().map(|(id, _)| id).collect();
    let core_units = core.iter().map(|u| u.0.clone()).filter(|id| !carries.contains(id)).collect();

    // 裝備：名次差越負越好
    let mut deltas = c.item_deltas;
    deltas.sort_unstable_by(|a, b| a.2.total_cmp(&b.2));
    let unit_items = core
        .iter()
        .chain(&flex)
        .map(|(u, _, _)| {
            let items: Vec<String> =
                deltas.iter().filter(|d| &d.0 == u).take(3).map(|d| d.1.clone()).collect();
            (u.clone(), items)
        })
        .collect();
    let mut item_tiers: Vec<(String, Vec<String>, f32)> = Vec::new();
    for (unit, item, d) in &deltas {
        if let Some(t) = item_tiers.iter_mut().find(|t| &t.0 == item) {
            if t.1.len() < 3 && !t.1.contains(unit) && *d < 0.0 {
                t.1.push(unit.clone());
            }
        } else if item_tiers.len() < 15 {
            item_tiers.push((item.clone(), vec![unit.clone()], *d as f32));
        }
    }

    // 等級分布：7 以下 / 8 / 9 以上
    let mut lv = [(0.0f64, 0.0f64); 3];
    for (level, cnt, sum) in &c.levels {
        let i = if *level <= 7.0 { 0 } else if *level < 9.0 { 1 } else { 2 };
        lv[i].0 += cnt;
        lv[i].1 += sum;
    }
    let lv_total: f64 = lv.iter().map(|l| l.0).sum::<f64>().max(1.0);
    let levels = ["7-", "8", "9+"]
        .iter()
        .zip(lv)
        .filter(|(_, l)| l.0 > 0.0)
        .map(|(lab, l)| (lab.to_string(), (l.0 / lv_total) as f32, (l.1 / l.0) as f32))
        .collect();

    let p_total: f64 = c.placement_distribution.iter().sum::<f64>().max(1.0);
    let placements = c.placement_distribution.iter().map(|p| (p / p_total) as f32).collect();

    let mut win_cons: Vec<(String, u8, f32, f32)> = c
        .units
        .iter()
        .filter(|u| u.2 / n >= 0.03)
        .map(|u| (u.0.clone(), u.1 as u8, share(u.2), u.3 as f32))
        .collect();
    win_cons.sort_unstable_by(|a, b| a.3.total_cmp(&b.3));
    win_cons.truncate(12);

    let mut traits_all = c.traits;
    traits_all.sort_unstable_by(|a, b| b.2.total_cmp(&a.2));
    let detail_traits: Vec<(String, u8, f32, f32)> =
        traits_all.iter().map(|t| (t.0.clone(), t.1 as u8, share(t.2), t.3 as f32)).collect();
    let traits = traits_all.into_iter().filter(|t| t.2 / n >= 0.5).map(|t| (t.0, t.1 as u8)).collect();

    let mut variants: Vec<Variant> = c.comps.into_iter().map(variant).collect();
    variants.sort_unstable_by(|a, b| b.count.cmp(&a.count));
    let lvl9 = c.lvl9_comps.into_iter().map(variant).collect();

    let top_players = c
        .top_players
        .iter()
        .filter_map(|p| {
            let a = p.as_array()?;
            Some((a.first()?.as_str()?.to_string(), a.get(1)?.as_str()?.to_string()))
        })
        .collect();

    CompRow {
        count: c.count,
        place: c.place,
        top4: c.top4 as f32 * 100.0 / n as f32,
        win: c.win as f32 * 100.0 / n as f32,
        lp_delta: c.lp_delta,
        carries,
        core_units,
        traits,
        detail: CompDetail {
            play_rate: c.count as f32 * 8.0 / total,
            core,
            flex,
            unit_items,
            item_tiers,
            carries: carries_w,
            levels,
            placements,
            win_cons,
            traits: detail_traits,
            variants,
            lvl9,
            top_players,
            code: c.code.unwrap_or_default(),
        },
    }
}

// ---------- Community Dragon zh_tw.json ----------

#[derive(Deserialize)]
struct RawCd {
    #[serde(default)]
    items: Vec<CdItem>,
    #[serde(default)]
    sets: HashMap<String, CdSet>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CdItem {
    api_name: Option<String>,
    name: Option<String>,
    icon: Option<String>,
    #[serde(default)]
    composition: Option<Vec<String>>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct CdSet {
    champions: Vec<CdChamp>,
    traits: Vec<CdTrait>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CdChamp {
    api_name: Option<String>,
    name: Option<String>,
    #[serde(default)]
    cost: Option<u8>,
    square_icon: Option<String>,
    tile_icon: Option<String>,
    #[serde(default)]
    traits: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CdTrait {
    api_name: Option<String>,
    name: Option<String>,
    icon: Option<String>,
    #[serde(default)]
    effects: Vec<CdEffect>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CdEffect {
    min_units: Option<u32>,
}

/// CDragon 的 `.tex` 資產路徑 → 可下載的 PNG 網址
pub fn cd_icon_url(path: &str) -> String {
    let p = path.to_ascii_lowercase();
    let p = p.strip_suffix(".tex").map(|s| format!("{s}.png")).unwrap_or(p);
    format!("https://raw.communitydragon.org/latest/game/{p}")
}

/// 目前最新賽季編號（用來抓對應的英文名稱檔）
pub fn latest_set(st: &Statics) -> Option<u32> {
    st.map.keys().filter_map(|k| k.strip_prefix("DA_")?.split('_').next()?.parse::<u32>().ok()).max()
}

/// tactics.tools 的 en.js：每行 `window.sNNXxxi18n = {...};`，取出 id → 英文名稱
pub fn apply_english(st: &mut Statics, js: &str) {
    for line in js.lines() {
        let (Some(a), Some(b)) = (line.find('{'), line.rfind('}')) else { continue };
        let Ok(map) = serde_json::from_str::<HashMap<String, serde_json::Value>>(&line[a..=b]) else { continue };
        for (k, v) in map {
            if k.ends_with("_desc") {
                continue;
            }
            if let (Some(name), Some(e)) = (v.as_str(), st.map.get_mut(&k)) {
                e.en = name.to_string();
            }
        }
    }
}

pub fn parse_statics(bytes: &[u8]) -> Result<Statics, String> {
    let raw: RawCd = serde_json::from_slice(bytes).map_err(|e| format!("靜態資料格式錯誤：{e}"))?;
    let mut map: HashMap<String, StaticEntry> = HashMap::new();

    for it in raw.items {
        let (Some(id), Some(name)) = (it.api_name, it.name) else { continue };
        let icon = it.icon.filter(|s| s.ends_with(".tex")).map(|s| cd_icon_url(&s)).unwrap_or_default();
        let parts = it.composition.unwrap_or_default();
        map.insert(id, StaticEntry { name, icon, parts, ..Default::default() });
    }

    // 依賽季編號由舊到新覆寫，最新賽季優先
    let mut sets: Vec<(u32, CdSet)> =
        raw.sets.into_iter().filter_map(|(k, v)| k.parse().ok().map(|n| (n, v))).collect();
    sets.sort_unstable_by_key(|(n, _)| *n);
    for (_, set) in sets {
        for c in set.champions {
            let (Some(id), Some(name)) = (c.api_name, c.name) else { continue };
            let icon = c.square_icon.or(c.tile_icon).map(|s| cd_icon_url(&s)).unwrap_or_default();
            let traits = c.traits.unwrap_or_default();
            map.insert(id, StaticEntry { name, cost: c.cost.unwrap_or(0), icon, traits, ..Default::default() });
        }
        for t in set.traits {
            let (Some(id), Some(name)) = (t.api_name, t.name) else { continue };
            let icon = t.icon.map(|s| cd_icon_url(&s)).unwrap_or_default();
            let tiers = t.effects.iter().map(|e| e.min_units.unwrap_or(0).min(255) as u8).collect();
            map.insert(id, StaticEntry { name, icon, tiers, ..Default::default() });
        }
    }
    Ok(Statics { fetched_at: now_secs(), map })
}

// ---------- 英雄／裝備詳細資料（stats2/unit、stats2/item） ----------

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct RawEntity {
    base: RawPerfRate,
    place_distribution: Vec<f64>,
    star_level_data: Vec<RawIdPerf>,
    item_count_data: Vec<RawIdPerf>,
    items: Vec<serde_json::Value>,
    item_pairs: Vec<serde_json::Value>,
    item_trios: Vec<serde_json::Value>,
    units: Vec<serde_json::Value>,
    traits: Vec<serde_json::Value>,
    region_stats: Vec<RawIdPerf>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawPerfRate {
    count: u32,
    rate: f32,
    place: f32,
    top4: f32,
    won: f32,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawIdPerf {
    id: serde_json::Value,
    count: u32,
    place: f32,
    top4: f32,
    won: f32,
}

fn perf_of(v: &serde_json::Value) -> Perf {
    let f = |k: &str| v.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
    Perf { count: f("count") as u32, place: f("place"), top4: f("top4"), won: f("won") }
}

fn strings(v: Option<&serde_json::Value>) -> Vec<String> {
    match v {
        Some(serde_json::Value::String(s)) => vec![s.clone()],
        Some(serde_json::Value::Array(a)) => a.iter().filter_map(|x| x.as_str().map(String::from)).collect(),
        _ => vec![],
    }
}

fn builds(list: &[serde_json::Value]) -> Vec<Build> {
    let mut v: Vec<Build> = list
        .iter()
        .map(|b| Build {
            items: strings(b.get("items")),
            perf: perf_of(b),
            delta: b.get("adjDelta").and_then(|x| x.as_f64()).unwrap_or(0.0) as f32,
        })
        .filter(|b| !b.items.is_empty())
        .collect();
    v.sort_unstable_by(|a, b| b.perf.count.cmp(&a.perf.count));
    v
}

fn rels(list: &[serde_json::Value]) -> Vec<Rel> {
    let mut v: Vec<Rel> = list
        .iter()
        .filter_map(|r| {
            let (id, tier) = match r.get("id")? {
                serde_json::Value::String(s) => (s.clone(), 0),
                serde_json::Value::Array(a) => {
                    (a.first()?.as_str()?.to_string(), a.get(1).and_then(|t| t.as_u64()).unwrap_or(0) as u8)
                }
                _ => return None,
            };
            let delta = r.get("delta").and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
            Some(Rel { id, tier, perf: perf_of(r), delta })
        })
        .collect();
    v.sort_unstable_by(|a, b| b.perf.count.cmp(&a.perf.count));
    v
}

fn id_perf(list: Vec<RawIdPerf>) -> Vec<(serde_json::Value, Perf)> {
    list.into_iter()
        .map(|r| (r.id, Perf { count: r.count, place: r.place, top4: r.top4, won: r.won }))
        .collect()
}

pub fn parse_entity(bytes: &[u8]) -> Result<EntityDetail, String> {
    let raw: RawEntity = serde_json::from_slice(bytes).map_err(|e| format!("詳細資料格式錯誤：{e}"))?;
    let p_total: f64 = raw.place_distribution.iter().sum::<f64>().max(1.0);
    let num = |v: &serde_json::Value| v.as_u64().unwrap_or(0) as u8;
    let mut stars: Vec<(u8, Perf)> = id_perf(raw.star_level_data).iter().map(|(i, p)| (num(i), *p)).collect();
    stars.sort_unstable_by_key(|s| s.0);
    let mut item_counts: Vec<(u8, Perf)> = id_perf(raw.item_count_data).iter().map(|(i, p)| (num(i), *p)).collect();
    item_counts.sort_unstable_by_key(|s| s.0);
    let mut regions: Vec<(String, Perf)> = id_perf(raw.region_stats)
        .into_iter()
        .map(|(i, p)| (i.as_str().unwrap_or("?").to_uppercase(), p))
        .collect();
    regions.sort_unstable_by(|a, b| b.1.count.cmp(&a.1.count));
    Ok(EntityDetail {
        base: Perf { count: raw.base.count, place: raw.base.place, top4: raw.base.top4, won: raw.base.won },
        rate: raw.base.rate,
        placements: raw.place_distribution.iter().map(|p| (p / p_total) as f32).collect(),
        stars,
        item_counts,
        builds1: builds(&raw.items),
        builds2: builds(&raw.item_pairs),
        builds3: builds(&raw.item_trios),
        units: rels(&raw.units),
        traits: rels(&raw.traits),
        items: if raw.items.first().is_some_and(|i| i.get("id").is_some()) { rels(&raw.items) } else { vec![] },
        regions,
    })
}
