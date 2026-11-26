//! 中英文模糊搜尋。
//!
//! 比對方式（任一成立即算符合，分數越高越相關）：
//! 1. 子字串：「殺手」→ 巨人殺手、「giant」→ Giant Slayer
//! 2. 英文字首縮寫：「gs」→ Giant Slayer、「ie」→ Infinity Edge
//! 3. 依序出現的字：「巨殺」→ 巨人殺手、「gslyr」→ Giant Slayer
//! 4. 打錯一個字（4 個字以上）：「teemp」→ Teemo、「giamt」→ Giant
//!
//! 查詢可用空白分成多個關鍵字，每個都要符合（例如「聖光 殺手」）。

use crate::model::Statics;

/// 去掉空白和標點並轉小寫
fn normalize(s: &str) -> Vec<char> {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// 英文字首：Giant Slayer → gs、MasterYi → my
fn initials(s: &str) -> Vec<char> {
    let mut out = Vec::new();
    let mut prev_alpha = false;
    let mut prev_lower = false;
    for c in s.chars() {
        // 撇號不切斷單字：Guinsoo's → g，不是 g + s
        if c == '\'' || c == '’' {
            continue;
        }
        let start_word = c.is_alphanumeric() && (!prev_alpha || (c.is_uppercase() && prev_lower));
        if start_word {
            out.extend(c.to_lowercase());
        }
        prev_alpha = c.is_alphanumeric();
        prev_lower = c.is_lowercase();
    }
    out
}

fn find(hay: &[char], needle: &[char]) -> Option<usize> {
    if needle.is_empty() || needle.len() > hay.len() {
        return None;
    }
    hay.windows(needle.len()).position(|w| w == needle)
}

/// 依序出現：回傳中間跳過的字數
fn subsequence_gaps(hay: &[char], needle: &[char]) -> Option<usize> {
    let mut it = hay.iter().enumerate();
    let mut gaps = 0;
    let mut last: Option<usize> = None;
    for n in needle {
        let (i, _) = it.find(|(_, h)| *h == n)?;
        if let Some(l) = last {
            gaps += i - l - 1;
        }
        last = Some(i);
    }
    Some(gaps)
}

/// 編輯距離是否 <= 1（長度差最多 1）
fn within_one_edit(a: &[char], b: &[char]) -> bool {
    let (la, lb) = (a.len(), b.len());
    if la.abs_diff(lb) > 1 {
        return false;
    }
    let (mut i, mut j, mut edits) = (0, 0, 0);
    while i < la && j < lb {
        if a[i] == b[j] {
            i += 1;
            j += 1;
            continue;
        }
        edits += 1;
        if edits > 1 {
            return false;
        }
        match la.cmp(&lb) {
            std::cmp::Ordering::Greater => i += 1,
            std::cmp::Ordering::Less => j += 1,
            std::cmp::Ordering::Equal => {
                i += 1;
                j += 1;
            }
        }
    }
    edits + (la - i) + (lb - j) <= 1
}

/// 單一關鍵字對單一字串的分數；不符合回傳 None
fn score_one(q: &[char], raw: &str) -> Option<i32> {
    let t = normalize(raw);
    if t.is_empty() {
        return None;
    }
    if t == q {
        return Some(1000);
    }
    // 短的英文查詢（2~3 字）優先當縮寫：「gs」→ Giant Slayer 排在 Warmogs 前面
    if (2..=3).contains(&q.len()) && q.iter().all(|c| c.is_ascii()) && initials(raw).starts_with(q) {
        return Some(900);
    }
    if let Some(pos) = find(&t, q) {
        return Some(800 - pos as i32 * 5 - (t.len() as i32 - q.len() as i32));
    }
    let ini = initials(raw);
    if q.len() >= 2 && ini.starts_with(q) {
        return Some(600);
    }
    // 中間的縮寫：「gs」→ Radiant Giant Slayer
    if q.len() >= 2 && find(&ini, q).is_some() {
        return Some(550);
    }
    if let Some(g) = subsequence_gaps(&t, q) {
        // 中文兩個字以上、英文三個字以上才用，避免太鬆
        let min = if q.iter().any(|c| !c.is_ascii()) { 2 } else { 3 };
        if q.len() >= min && g <= q.len() * 3 {
            return Some(400 - g as i32 * 10);
        }
    }
    if q.len() >= 4 {
        let n = q.len();
        for w in (n.saturating_sub(1)..=n + 1).filter(|&w| w > 0 && w <= t.len()) {
            if t.windows(w).any(|win| within_one_edit(win, q)) {
                return Some(200);
            }
        }
    }
    None
}

/// 查詢（可多個關鍵字）對多個名稱（中文、英文、id）的分數；每個關鍵字都要符合
pub fn score(query: &str, names: &[&str]) -> Option<i32> {
    let mut total = 0;
    let mut any = false;
    for word in query.split_whitespace() {
        let q = normalize(word);
        if q.is_empty() {
            continue;
        }
        any = true;
        total += names.iter().filter_map(|n| score_one(&q, n)).max()?;
    }
    Some(if any { total } else { 0 })
}

pub fn matches(query: &str, names: &[&str]) -> bool {
    query.trim().is_empty() || score(query, names).is_some()
}

/// 英雄／裝備／羈絆：用中文名、英文名和 id 一起比對
pub fn matches_id(query: &str, st: &Statics, id: &str) -> bool {
    matches(query, &[st.name(id), st.en(id), id])
}

pub fn score_id(query: &str, st: &Statics, id: &str) -> i32 {
    score(query, &[st.name(id), st.en(id), id]).unwrap_or(0)
}

/// 有搜尋字時，依相關度重新排序（同分維持原本的排序）
pub fn rank(idx: &mut [usize], query: &str, score_of: impl Fn(usize) -> i32) {
    if !query.trim().is_empty() {
        idx.sort_by_cached_key(|&i| std::cmp::Reverse(score_of(i)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(q: &str, name: &str) -> bool {
        score(q, &[name]).is_some()
    }

    #[test]
    fn fuzzy() {
        assert!(hit("殺手", "巨人殺手"));
        assert!(hit("巨殺", "巨人殺手"));
        assert!(hit("聖光 殺手", "聖光巨人殺手"));
        assert!(!hit("聖光 殺手", "巨人殺手"));
        assert!(hit("giant", "Giant Slayer"));
        assert!(hit("GIANT slayer", "Giant Slayer"));
        assert!(hit("gs", "Giant Slayer"));
        assert!(hit("gslyr", "Giant Slayer"));
        assert!(hit("teemp", "Teemo"));
        assert!(hit("giamt", "Giant Slayer"));
        assert!(hit("my", "MasterYi"));
        assert!(hit("gs", "Radiant Giant Slayer"));
        assert!(score("gs", &["Giant Slayer"]) > score("gs", &["Radiant Giant Slayer"]));
        assert!(score("gs", &["Giant Slayer"]) > score("gs", &["Warmogs Armor"]));
        assert!(score("gs", &["Giant Slayer"]) > score("gs", &["Guinsoo's Rageblade"]));
        assert!(!hit("xyz", "Teemo"));
        assert!(!hit("提", "巨人殺手"));
        // 精確比對分數要比模糊高
        assert!(score("teemo", &["Teemo"]) > score("teemp", &["Teemo"]));
    }
}
