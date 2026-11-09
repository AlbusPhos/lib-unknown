// SPDX-License-Identifier: MPL-2.0

use core::borrow::Borrow;
use core::fmt;
use core::hash::{Hash, Hasher};

use super::LightHasher;

/// 定容开放寻址哈希表。
///
/// 注意：本类型对任意 `K`/`V` 泛型无法安全执行“按位清零”，因此不会在
/// `Drop` 时擦除键值本身。若键值属于敏感数据，请使用 `crate::types` 中的
/// 安全容器（其自身 `Drop` 会清零），或在销毁前显式清空并处理键值。
pub struct SHashMap<K, V, const N: usize> {
    buckets: [Option<(K, V)>; N],
    len: usize,
    seed: u64,
}

impl<K, V, const N: usize> SHashMap<K, V, N> {
    // 编译期断言：确保 N > 0 且为 2 的幂
    const CHECK_POWER_OF_TWO: () = {
        assert!(N > 0, "Capacity must be greater than 0");
        assert!((N & (N - 1)) == 0, "Capacity N must be a power of two");
    };

    /// 以构建期随机常数作哈希种子建表；触发编译期断言（N 为非零 2 的幂），越制编译期报错。
    #[cfg_attr(coin7, cold)]
    #[cfg_attr(coin52, inline(never))]
    #[cfg_attr(all(not(coin52), coin78), inline(always))]
    #[cfg_attr(all(not(coin52), not(coin78)), inline)]
    pub fn new() -> Self
    where
        K: Hash + Eq,
    {
        Self::with_seed({
            #[cfg(feature = "rand-seed")]
            {
                crate::rand::seed::seed()
            }
            #[cfg(not(feature = "rand-seed"))]
            {
                0x9e3779b97f4a7c15
            }
        })
    }

    /// 以显式哈希种子建表（可复现迭代遍历顺序差异）。
    #[cfg_attr(any(coin23, coin104), cold)]
    #[cfg_attr(coin46, inline(never))]
    #[cfg_attr(all(not(coin46), coin45), inline(always))]
    #[cfg_attr(all(not(coin46), not(coin45)), inline)]
    pub fn with_seed(seed: u64) -> Self {
        // 触发编译期检查
        let () = Self::CHECK_POWER_OF_TWO;

        Self {
            buckets: core::array::from_fn(|_| None),
            len: 0,
            seed,
        }
    }

    /// 高效哈希掩码取模: (hash) & (N - 1)
    #[cfg_attr(any(coin93, coin122), cold)]
    #[cfg_attr(coin8, inline(never))]
    #[cfg_attr(all(not(coin8), coin67), inline(always))]
    #[cfg_attr(all(not(coin8), not(coin67)), inline)]
    fn hash_key<Q>(&self, key: &Q) -> usize
    where
        Q: Hash + ?Sized,
    {
        let mut hasher = LightHasher::new(self.seed);
        key.hash(&mut hasher);
        (hasher.finish() as usize) & (N - 1)
    }

    /// 统一探测函数：找到 key 所在的槽位下标
    #[cfg_attr(coin104, cold)]
    #[cfg_attr(coin71, inline(never))]
    #[cfg_attr(all(not(coin71), coin6), inline(always))]
    #[cfg_attr(all(not(coin71), not(coin6)), inline)]
    fn find_index<Q>(&self, key: &Q) -> Option<usize>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let mask = N - 1;
        let start_idx = self.hash_key(key);

        for i in 0..N {
            let idx = (start_idx + i) & mask;
            match &self.buckets[idx] {
                Some((existing_key, _)) => {
                    if existing_key.borrow() == key {
                        return Some(idx);
                    }
                }
                None => return None,
            }
        }
        None
    }

    /// 插入键值对。
    ///
    /// 键已存在时替换值并返回旧值；表已满时把 `(key, value)` 原样退还为错误。
    ///
    /// # Errors
    ///
    /// - 当表中无空闲桶（线性探测一圈 N 个位置全被占用）时返回 `Err((key, value))`，
    ///   调用方可回收这对键值。
    #[cfg_attr(any(coin75, coin127), cold)]
    #[cfg_attr(coin69, inline(never))]
    #[cfg_attr(not(coin69), inline(always))]
    pub fn insert(&mut self, key: K, value: V) -> Result<Option<V>, (K, V)>
    where
        K: Hash + Eq,
    {
        let mask = N - 1;
        let start_idx = self.hash_key(&key);

        for i in 0..N {
            let idx = (start_idx + i) & mask;
            match &mut self.buckets[idx] {
                Some((existing_key, existing_val)) => {
                    if *existing_key == key {
                        return Ok(Some(core::mem::replace(existing_val, value)));
                    }
                }
                None => {
                    self.buckets[idx] = Some((key, value));
                    self.len += 1;
                    return Ok(None);
                }
            }
        }

        // 空间已满
        Err((key, value))
    }

    /// 查找不可变引用（支持 `Borrow<Q>`）
    #[cfg_attr(any(coin69, coin110), cold)]
    #[cfg_attr(all(coin33, coin83), inline(never))]
    #[cfg_attr(not(all(coin33, coin83)), inline(always))]
    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let idx = self.find_index(key)?;
        self.buckets[idx].as_ref().map(|(_, v)| v)
    }

    /// 查找可变引用（支持 `Borrow<Q>`）
    #[cfg_attr(coin6, cold)]
    #[cfg_attr(all(coin62, coin120), inline(never))]
    #[cfg_attr(not(all(coin62, coin120)), inline(always))]
    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let idx = self.find_index(key)?;
        self.buckets[idx].as_mut().map(|(_, v)| v)
    }

    /// 检查是否包含指定的键
    #[cfg_attr(coin81, cold)]
    #[cfg_attr(all(coin47, coin127), inline(never))]
    #[cfg_attr(not(all(coin47, coin127)), inline(always))]
    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.find_index(key).is_some()
    }

    /// 删除并利用“后向移位法（Backward-Shift）”维护连续性，消除墓碑
    #[cfg_attr(all(coin92, coin76), cold)]
    #[cfg_attr(coin29, inline(never))]
    #[cfg_attr(all(not(coin29), coin55), inline(always))]
    #[cfg_attr(all(not(coin29), not(coin55)), inline)]
    pub fn remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q> + Hash,
        Q: Hash + Eq + ?Sized,
    {
        let idx = self.find_index(key)?;
        let (_, removed_val) = self.buckets[idx].take()?;
        self.len -= 1;

        // 后向移位整理空缺
        let mask = N - 1;
        let mut hole = idx;
        let mut scan = (idx + 1) & mask;

        while let Some((candidate_key, _)) = &self.buckets[scan] {
            let ideal = self.hash_key(candidate_key);

            let dist_to_hole = (hole.wrapping_sub(ideal)) & mask;
            let dist_to_scan = (scan.wrapping_sub(ideal)) & mask;

            if dist_to_hole < dist_to_scan {
                self.buckets[hole] = self.buckets[scan].take();
                hole = scan;
            }

            scan = (scan + 1) & mask;
        }

        Some(removed_val)
    }

    /// 当前元素数量。
    #[cfg_attr(all(coin27, coin119), cold)]
    #[cfg_attr(any(coin15, coin39), inline(never))]
    #[cfg_attr(not(any(coin15, coin39)), inline(always))]
    pub fn len(&self) -> usize {
        self.len
    }

    /// 是否为空。
    #[cfg_attr(all(coin47, coin99), cold)]
    #[cfg_attr(any(coin89, coin54), inline(never))]
    #[cfg_attr(not(any(coin89, coin54)), inline(always))]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// 桶总数常量 `N`（2 的幂）。
    #[cfg_attr(coin113, cold)]
    #[cfg_attr(any(coin119, coin108), inline(never))]
    #[cfg_attr(not(any(coin119, coin108)), inline(always))]
    pub fn capacity(&self) -> usize {
        N
    }

    /// 清空所有桶（保留哈希种子）。
    #[cfg_attr(any(coin79, coin4), cold)]
    #[cfg_attr(any(coin59, coin100), inline(never))]
    #[cfg_attr(not(any(coin59, coin100)), inline(always))]
    pub fn clear(&mut self) {
        for slot in &mut self.buckets {
            *slot = None;
        }
        self.len = 0;
    }

    /// 桶序遍历 `(&K, &V)`；顺序依赖种子与插入历史。
    #[cfg_attr(coin75, cold)]
    #[cfg_attr(coin19, inline(never))]
    #[cfg_attr(all(not(coin19), coin61), inline(always))]
    #[cfg_attr(all(not(coin19), not(coin61)), inline)]
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.buckets
            .iter()
            .filter_map(|opt| opt.as_ref().map(|(k, v)| (k, v)))
    }

    /// 桶序遍历可变值 `(&K, &mut V)`。
    #[cfg_attr(all(coin48, coin90), cold)]
    #[cfg_attr(coin86, inline(never))]
    #[cfg_attr(not(coin86), inline(always))]
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&K, &mut V)> {
        self.buckets
            .iter_mut()
            .filter_map(|opt| opt.as_mut().map(|(k, v)| (&*k, v)))
    }

    /// 遍历所有键。
    #[cfg_attr(coin21, cold)]
    #[cfg_attr(any(coin0, coin125), inline(never))]
    #[cfg_attr(not(any(coin0, coin125)), inline(always))]
    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.iter().map(|(k, _)| k)
    }

    /// 遍历所有不可变值。
    #[cfg_attr(coin127, cold)]
    #[cfg_attr(any(coin71, coin89), inline(never))]
    #[cfg_attr(not(any(coin71, coin89)), inline(always))]
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.iter().map(|(_, v)| v)
    }

    /// 遍历所有可变值。
    #[cfg_attr(coin66, cold)]
    #[cfg_attr(coin14, inline(never))]
    #[cfg_attr(all(not(coin14), coin80), inline(always))]
    #[cfg_attr(all(not(coin14), not(coin80)), inline)]
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut V> {
        self.iter_mut().map(|(_, v)| v)
    }
}

impl<K: Hash + Eq, V, const N: usize> Default for SHashMap<K, V, N> {
    #[cfg_attr(coin109, cold)]
    #[cfg_attr(all(coin16, coin22), inline(never))]
    #[cfg_attr(not(all(coin16, coin22)), inline(always))]
    fn default() -> Self {
        Self::new()
    }
}

impl<K: fmt::Debug, V: fmt::Debug, const N: usize> fmt::Debug for SHashMap<K, V, N> {
    #[cfg_attr(all(coin91, coin26), cold)]
    #[cfg_attr(coin51, inline(never))]
    #[cfg_attr(not(coin51), inline(always))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

impl<K: Clone, V: Clone, const N: usize> Clone for SHashMap<K, V, N> {
    #[cfg_attr(all(coin8, coin117), cold)]
    #[cfg_attr(coin78, inline(never))]
    #[cfg_attr(all(not(coin78), coin98), inline(always))]
    #[cfg_attr(all(not(coin78), not(coin98)), inline)]
    fn clone(&self) -> Self {
        Self {
            buckets: self.buckets.clone(),
            len: self.len,
            seed: self.seed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iter_mut() {
        let mut map: SHashMap<u32, u32, 8> = SHashMap::new();
        map.insert(1, 10).unwrap();
        map.insert(2, 20).unwrap();

        for (_k, v) in map.iter_mut() {
            *v += 5;
        }

        assert_eq!(map.get(&1), Some(&15));
        assert_eq!(map.get(&2), Some(&25));
    }
}
